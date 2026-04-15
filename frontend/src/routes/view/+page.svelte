<script lang="ts">
    import { page } from "$app/state";

  const ICE_CONFIG = {
    iceServers: [
      { urls: 'stun:stun.l.google.com:19302' },
      { urls: 'stun:stun1.l.google.com:19302' },
    ]
  };

  const streamer = $derived(page.url.searchParams.get('streamer') || '');

  let preview: HTMLVideoElement;

  const socket = new WebSocket(`https://${page.url.host}/ws`);
  let pc = $state<RTCPeerConnection>();

  socket.addEventListener('error', (data) => {
    console.error(data);
    alert("ERROR");
    window.location.reload();
  });

  socket.addEventListener('close', () => { window.location.reload() });

  socket.addEventListener('message', async (event) => {
    let commandStr: string = event.data.toString();
      const parts = commandStr.split(":");
      const command = parts.shift();
      const params = parts.join(":");
      switch (command) {
        case 'f': {
          const paramsSplit = params.split(":");
          paramsSplit.shift();
          const peerConnLocalDescription = JSON.parse(paramsSplit.join(":"));
          pc = new RTCPeerConnection(ICE_CONFIG);

          pc.ontrack = (e) => {
            if (preview.srcObject !== e.streams[0]) {
              preview.srcObject = e.streams[0];
              const videoData = e.streams[0].getVideoTracks()[0].getSettings();
              preview.style.width = videoData.width + 'px';
              preview.style.height = videoData.height + 'px';
              console.log(`Stream size: Width: ${videoData.width} height: ${videoData.height}`)
              preview.play();
            }
          };

          pc.onicecandidate = ({ candidate }) => {
            if (candidate) socket.send(`h:${streamer}:${JSON.stringify(candidate)}`);
          };

          pc.onconnectionstatechange = () => {
            if (!pc) return;
            const s = pc.connectionState;
            if (s === 'failed') pc.close();
          };

          await pc.setRemoteDescription(new RTCSessionDescription(peerConnLocalDescription));
          const answer = await pc.createAnswer();
          await pc.setLocalDescription(answer);
          socket.send(`f:${streamer}:${JSON.stringify(pc.localDescription)}`);
          break;
        }
      }
  })
</script>

<video id="video" autoplay playsinline bind:this={preview}></video>