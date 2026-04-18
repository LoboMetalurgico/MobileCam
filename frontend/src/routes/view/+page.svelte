<script lang="ts">
  import { page } from "$app/state";

  const ICE_CONFIG = {
    iceServers: [
      { urls: "stun:stun.l.google.com:19302" },
      { urls: "stun:stun1.l.google.com:19302" },
    ],
  };

  let streamer = $derived(page.url.searchParams.get("streamer") || "");
  let preview: HTMLVideoElement;

  $effect(() => {
    const wsProtocol = page.url.protocol === "https:" ? "wss:" : "ws:";
    const internalSocket = new WebSocket(
      `${wsProtocol}//${page.url.host}/ws?role=viewer&watch_id=${streamer}`,
    );

    let internalPc: RTCPeerConnection | undefined = undefined;
    let candidateQueue: RTCIceCandidateInit[] = [];

    internalSocket.addEventListener("error", (err) =>
      console.error("WebSocket Error:", err),
    );

    internalSocket.addEventListener("message", async (event) => {
      const commandStr: string = event.data.toString();
      const parts = commandStr.split(":");
      const command = parts.shift();
      const params = parts.join(":");

      switch (command) {
        case "f": {
          // Received Offer
          const paramsSplit = params.split(":");
          paramsSplit.shift();
          const peerConnRemoteDescription = JSON.parse(
            paramsSplit.join(":").slice(1),
          );

          if (internalPc) internalPc.close();

          internalPc = new RTCPeerConnection(ICE_CONFIG);

          // FIX: Simplified track handling
          internalPc.ontrack = (e) => {
            console.log("Track received!", e.track.kind);
            if (preview.srcObject !== e.streams[0]) {
              preview.srcObject = e.streams[0];
              preview
                .play()
                .catch((err) => console.error("Error playing video:", err));
            }
          };

          internalPc.onicecandidate = ({ candidate }) => {
            if (candidate)
              internalSocket.send(
                `h:${streamer}:#${JSON.stringify(candidate)}`,
              );
          };

          // DIAGNOSTICS: Monitor the connection state
          internalPc.onconnectionstatechange = () => {
            if (!internalPc) return;
            console.log("WebRTC Connection State:", internalPc.connectionState);
            if (internalPc.connectionState === "failed") {
              console.error(
                "WebRTC connection failed. A TURN server might be required.",
              );
              internalPc.close();
            }
          };

          await internalPc.setRemoteDescription(
            new RTCSessionDescription(peerConnRemoteDescription),
          );

          const answer = await internalPc.createAnswer();
          await internalPc.setLocalDescription(answer);

          internalSocket.send(
            `f:${streamer}:#${JSON.stringify(internalPc.localDescription)}`,
          );

          while (candidateQueue.length > 0) {
            const queuedCandidate = candidateQueue.shift();
            if (queuedCandidate) {
              await internalPc.addIceCandidate(
                new RTCIceCandidate(queuedCandidate),
              );
            }
          }
          break;
        }

        case "g": {
          // Received ICE Candidate
          const candidateString = params.split(":");
          candidateString.shift();
          const candidate = JSON.parse(candidateString.join(":").slice(1));

          if (
            internalPc &&
            internalPc.remoteDescription &&
            internalPc.remoteDescription.type
          ) {
            await internalPc.addIceCandidate(new RTCIceCandidate(candidate));
          } else {
            candidateQueue.push(candidate);
          }
          break;
        }
      }
    });

    return () => {
      internalSocket.close();
      if (internalPc) internalPc.close();
    };
  });
</script>

<video id="video" autoplay playsinline bind:this={preview}></video>

<style>
  * {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
  }

  video {
    width: 100vw;
    height: 100vh;
    position: absolute;
    top: 0;
    left: 0;
    object-fit: cover;
    background-color: #111;
  }
</style>
