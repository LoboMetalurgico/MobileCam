<script lang="ts">
  import { page } from "$app/state";
  import type { Quality } from "$lib/interfaces/Quality";
  import { toggleFullScreen } from "$lib/utils/phoneUtils";
  import Camera from "./camera.svelte";
  import Screen from "./screen.svelte";

  function stringToQuality(string: string): Quality {
    if (string === "3") return "low";
    if (string === "2") return "medium";
    return "high";
  }

  const ICE_CONFIG = {
    iceServers: [
      { urls: "stun:stun.l.google.com:19302" },
      { urls: "stun:stun1.l.google.com:19302" },
    ],
  };

  // Hoisted state
  let peerConnections: Record<string, RTCPeerConnection> = {};
  let candidateQueues: Record<string, RTCIceCandidateInit[]> = {}; // Queue for the race condition
  let ws: WebSocket | null = $state(null);
  // let sessionId: number | null = $state(null);
  // svelte-ignore non_reactive_update
  let sourceMedia: Camera | Screen;
  let isCamera = $state(true);

  function setStreamBandwidth(
    peerConnection: RTCPeerConnection,
    maxBitrate: number,
    maxFramerate: number,
  ) {
    for (const sender of peerConnection.getSenders()) {
      if (sender.track?.kind !== "video") continue;
      const params = sender.getParameters();
      if (!params.encodings?.length) params.encodings = [{}];
      params.encodings[0].maxBitrate = maxBitrate;
      params.encodings[0].maxFramerate = maxFramerate;
      sender.setParameters(params).catch(() => {});
    }
  }

  function setStreamingBandwidth(maxBitrate: number, maxFramerate: number) {
    for (const pc of Object.values(peerConnections)) {
      setStreamBandwidth(pc, maxBitrate, maxFramerate);
    }
  }

  function onCameraTransform(transform: { zoom: number; rotation: number }) {
    if (!ws) return;
    ws.send(`j:#${JSON.stringify(transform)}`);
  }

  async function createPeerConnection(viewerSocketId: string) {
    const localStream = sourceMedia.getVideoStream();
    if (!localStream) return;
    if (peerConnections[viewerSocketId]) {
      peerConnections[viewerSocketId].close();
    }

    const pc = new RTCPeerConnection(ICE_CONFIG);
    peerConnections[viewerSocketId] = pc;
    candidateQueues[viewerSocketId] = []; // Initialize queue for this viewer

    for (const track of localStream.getTracks()) {
      pc.addTrack(track, localStream);
    }

    const bandwidth = sourceMedia.getCurrentBandwidth();
    setStreamBandwidth(pc, bandwidth.maxBitrate, bandwidth.maxFramerate);

    pc.onicecandidate = ({ candidate }) => {
      if (candidate) {
        ws?.send(`g:${viewerSocketId}:#${JSON.stringify(candidate)}`);
      }
    };

    pc.onconnectionstatechange = () => {
      if (pc.connectionState !== "failed") return;
      delete peerConnections[viewerSocketId];
      delete candidateQueues[viewerSocketId];
      pc.close();
    };

    return pc;
  }

  async function createOffer(viewerId: string) {
    const localStream = sourceMedia.getVideoStream();
    if (!localStream) return;
    const pc = await createPeerConnection(viewerId);
    if (!pc) return;

    const transceivers = pc.getTransceivers();
    const videoTransceiver = transceivers.find(
      (t) => t.sender.track && t.sender.track.kind === "video",
    );

    if (videoTransceiver) {
      const capabilities = RTCRtpReceiver.getCapabilities("video");
      const h264Codecs = (capabilities ?? { codecs: [] }).codecs.filter(
        (codec) => codec.mimeType.toLowerCase() === "video/h264",
      );

      if (h264Codecs.length > 0) {
        videoTransceiver.setCodecPreferences(h264Codecs);
      } else {
        console.warn("H.264 is not supported by this browser.");
      }
    }

    const offer = await pc.createOffer();
    await pc.setLocalDescription(offer);
    ws?.send(`f:${viewerId}:#${JSON.stringify(pc.localDescription)}`);
  }

  async function init() {
    const wsProtocol = page.url.protocol === "https:" ? "wss:" : "ws:";
    ws = new WebSocket(`${wsProtocol}//${page.url.host}/ws?role=streamer`);
    ws.onopen = async () => {
      await sourceMedia.start();
    };
  }

  $effect(() => {
    if (!ws) return;
    ws.addEventListener("error", (err) =>
      console.error("WebSocket Error:", err),
    );

    ws.addEventListener("close", () => {
      // window.location.reload();
    });

    ws.addEventListener("message", async (event) => {
      let commandStr: string = event.data.toString();
      const parts = commandStr.split(":");
      const command = parts.shift();
      const params = parts.join(":");

      switch (command) {
        case "e": // Viewer requests offer
          await createOffer(params);
          break;

        case "h": {
          // Received ICE candidate from Viewer
          const candidateString = params.split(":");
          const viewerId = candidateString.shift();
          if (!viewerId) return;

          const candidate = JSON.parse(candidateString.join(":").slice(1));
          const pc = peerConnections[viewerId];

          if (pc && candidate) {
            // QUEUE LOGIC: Only add if remote description is set
            if (pc.remoteDescription && pc.remoteDescription.type) {
              await pc.addIceCandidate(new RTCIceCandidate(candidate));
            } else {
              console.log(`Queueing ICE candidate for ${viewerId}`);
              candidateQueues[viewerId].push(candidate);
            }
          }
          break;
        }

        case "f": {
          // Received Answer from Viewer
          const candidateString = params.split(":");
          const viewerId = candidateString.shift();
          if (!viewerId) return;

          const sdp = JSON.parse(candidateString.join(":").slice(1));
          const pc = peerConnections[viewerId];
          console.log(
            `[SIGNALING] Got Answer. Looking for PC with ID: ${viewerId}. Did we find it?`,
            !!pc,
          );

          if (pc && pc.signalingState !== "stable") {
            await pc.setRemoteDescription(new RTCSessionDescription(sdp));

            // QUEUE LOGIC: Flush the queue now that description is set
            const queue = candidateQueues[viewerId] || [];
            while (queue.length > 0) {
              const queuedCandidate = queue.shift();
              if (queuedCandidate) {
                await pc.addIceCandidate(new RTCIceCandidate(queuedCandidate));
              }
            }
          }
          break;
        }

        case "i": {
          // Apply quality change
          await sourceMedia.applyQuality(stringToQuality(params.split(":")[1]));
          break;
        }

        case "l": {
          // Set session ID
          // sessionId = parseInt(params);
          break;
        }
      }
    });

    return () => {
      ws?.close();
      for (const pc of Object.values(peerConnections)) {
        pc.close();
      }
      peerConnections = {};
      candidateQueues = {};
    };
  });
</script>

{#if !ws}
  <div class="selectionScreen">
    <button
      id="start-btn"
      class="selectionButton clickable"
      onclick={() => {
        isCamera = true;
        init();
      }}
    >
      <div class="buttonIcon camera"></div>
      <span class="selectionTitle">Start Camera</span>
    </button>
    <button
      id="start-btn"
      class="selectionButton clickable"
      onclick={() => {
        isCamera = false;
        init();
      }}
    >
      <div class="buttonIcon screen"></div>
      <span class="selectionTitle">Start Screen Share</span>
    </button>
  </div>
{:else}
  <div class="overlay">
    <button
      class="fullscreenBtn clickable"
      onclick={(e) => {
        const state = toggleFullScreen();
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (e.target as any).classList.toggle("isFullscreen", state);
      }}
      aria-label="Tela Cheia"><div class="fullscreenIcon"></div></button
    >
    <img class="branding" alt="Company Logo" src="/image/logo.png" />
  </div>
  {#if isCamera}
    <Camera
      bind:this={sourceMedia}
      onTransform={onCameraTransform}
      {setStreamingBandwidth}
    />
  {:else}
    <Screen
      bind:this={sourceMedia}
      {setStreamingBandwidth}
      onTransform={onCameraTransform}
    />
  {/if}
{/if}

<style>
  .selectionScreen {
    display: grid;
    grid-template-columns: 2fr 1fr;
    grid-template-rows: 1fr;
    width: 100%;
    height: 100%;
    justify-items: center;
    align-items: center;
    gap: 1rem;
    padding: 1rem;

    @media screen and (orientation: portrait) {
      grid-template-rows: 2fr 1fr;
      grid-template-columns: 1fr;
    }
  }

  .selectionButton {
    cursor: pointer;
    border: 1px solid var(--accent-color);
    background: hsla(from var(--accent-color) h s l / 0.3);
    color: var(--accent-color);
    font-size: 1.5rem;
    padding: 1.5rem 0.75rem;
    border-radius: 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    width: 100%;
    height: 100%;
  }

  .buttonIcon {
    height: 100%;
    width: 100%;
    background: var(--accent-color);
    mask-size: contain;
    mask-repeat: no-repeat;
    mask-position: center center;
    &.camera {
      mask-image: url("/icons/camera.svg");
    }
    &.screen {
      mask-image: url("/icons/screenshare.svg");
    }
  }

  .selectionTitle {
    font-size: 1.5rem;
    font-weight: bolder;
  }

  .overlay {
    position: absolute;
    width: 100%;
    height: 100%;
    padding: 1rem;
    display: flex;
    flex-direction: row;
    justify-content: space-between;
    align-items: start;
    z-index: 9999;
    pointer-events: none;
  }

  .fullscreenBtn {
    width: 3.5rem;
    height: auto;
    aspect-ratio: 1;
    background: hsla(from var(--accent-color) h s l / 0.3);
    border-radius: 0.5rem;
    border: 1px solid var(--accent-color);
    padding: 0.25rem;
    cursor: pointer;
    position: absolute;
    top: 1rem;
    left: 1rem;
  }

  .fullscreenIcon {
    width: 100%;
    height: 100%;
    background: var(--accent-color);
    mask-image: url("/icons/fullscreenicon.svg");
    mask-size: contain;
  }
  .fullscreenIcon:global(.isFullscreen) {
    mask-image: url("/icons/fullscreenexit.svg");
  }

  .clickable {
    pointer-events: all;
  }

  .branding {
    pointer-events: none;
    opacity: 0.35;
    width: 45dvw;
    height: 45dvh;
    object-fit: contain;
    position: absolute;
    bottom: 2rem;
    right: 2rem;
  }
</style>
