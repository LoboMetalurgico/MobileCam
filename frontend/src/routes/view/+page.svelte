<script lang="ts">
  import { page } from "$app/state";
  import { Role } from "$lib/interfaces/Role";
  import { WRTCManager } from "$lib/WebRTCManager";
  import { WebsocketManager } from "$lib/websocketManager";
  import { onDestroy } from "svelte";

  let streamer = $derived(page.url.searchParams.get("streamer") || "");
  let isMuted = $state(false);
  let preview: HTMLVideoElement;
  const rtcManager = new WRTCManager({
    onRemoteTrack: (track, streams) => {
      if (preview.srcObject !== streams[0]) {
        preview.srcObject = streams[0];
        viewFallbackTransform.aspectRatio =
          track.getSettings().aspectRatio ?? 16 / 9;
        preview
          .play()
          .catch((err) => console.error("Error playing video:", err));
      }
      socket.send(`i:${streamer}:1`);
    },

    onICECandidate: (_, candidate) => {
      socket?.send(`h:${streamer}:#${JSON.stringify(candidate)}`);
    },
  });
  let socket = new WebsocketManager(Role.Viewer, commandHandler);
  let viewFallbackTransform = $state<{
    rotation: number;
    zoom: number;
    aspectRatio: number;
  }>({ rotation: 0, zoom: 1, aspectRatio: 16 / 9 });

  $effect(() => {
    socket.connect({
      protocol: page.url.protocol,
      hostname: page.url.host,
      extraQueries: [["watch_id", streamer]],
    });
  });

  async function commandHandler(command: string, params: string) {
    switch (command) {
      case "f": {
        const paramsSplit = params.split(":");
        const streamerId = paramsSplit.shift();
        if (streamerId == null) {
          console.error("Streamer ID missing in offer command");
          return;
        }
        const peerConnRemoteDescription = JSON.parse(
          paramsSplit.join(":").slice(1),
        );

        const conn = rtcManager.getPeerConnection(streamerId);
        if (conn) conn.close();

        rtcManager.createPeerConnection(streamerId);

        await rtcManager.setRemoteDescription(
          streamerId,
          peerConnRemoteDescription,
        );
        const answer = await rtcManager.createStreamAnswer(streamerId);
        socket?.send(`f:${streamer}:#${JSON.stringify(answer)}`);
        break;
      }

      case "g": {
        // Received ICE Candidate
        const candidateString = params.split(":");
        candidateString.shift();
        const candidate = JSON.parse(candidateString.join(":").slice(1));
        await rtcManager.addICECandidate(streamer, candidate);
        break;
      }

      case "j": {
        const { zoom, rotation } = JSON.parse(params.slice(1));
        viewFallbackTransform.rotation = rotation;
        viewFallbackTransform.zoom = zoom;
        break;
      }
    }
  }

  onDestroy(() => {
    socket?.finish();
  });

  function toggleMute() {
    preview.muted = !preview.muted;
    isMuted = preview.muted;
  }
</script>

<div class="watcher">
  <div class="overlay">
    <button class="muteButton" onclick={toggleMute} aria-label="mute">
      <div class={`muteIcon ${isMuted ? "isMuted" : ""}`}></div>
    </button>
  </div>
  <div
    class="playerContainer"
    style={`--aspect-ratio: ${viewFallbackTransform.aspectRatio};`}
  >
    <video
      id="video"
      autoplay
      playsinline
      bind:this={preview}
      style={`--zoom: ${viewFallbackTransform.zoom};--rotate: ${viewFallbackTransform.rotation}deg;`}
    ></video>
  </div>
</div>

<style>
  .watcher {
    width: 100%;
    height: 100%;
    display: flex;
    position: relative;
  }

  .overlay {
    position: absolute;
    top: 0;
    left: 0;
    padding: 1rem;
    z-index: 99;
    opacity: 0;
    transition: opacity 0.25s ease;
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: row;
    gap: 1rem;
    justify-content: space-between;
    align-items: start;
  }

  .overlay:hover {
    opacity: 1;
  }

  .muteButton {
    width: 4rem;
    height: 4rem;
    border-radius: 0.5rem;
    border: 1px solid var(--accent-color);
    background: hsla(from var(--accent-color) h s l / 0.3);
    padding: 0.25rem;
  }

  .muteIcon {
    aspect-ratio: 1;
    height: 100%;
    width: auto;
    background: var(--accent-color);
    mask-size: contain;
    mask-position: center;
    mask-image: url("/icons/unmute.svg");
    &.isMuted {
      mask-image: url("/icons/mute.svg");
    }
  }

  .playerContainer {
    position: absolute;
    width: 100dvw;
    height: 100dvh;
    top: 0;
    left: 0;
    overflow: hidden;
    display: flex;
    justify-content: center;
    align-items: center;
    aspect-ratio: var(--aspect-ratio);
  }

  #video {
    width: 100%;
    height: 100%;
    object-fit: contain;
    background-color: #111;
    scale: var(--zoom);
    rotate: var(--zoom);
  }
</style>
