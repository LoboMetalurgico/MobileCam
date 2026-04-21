<script lang="ts">
  let isMuted = $state(true);
  let preview: HTMLVideoElement;
  let viewFallbackTransform = $state<{
    rotation: number;
    zoom: number;
    aspectRatio: number;
  }>({ rotation: 0, zoom: 0, aspectRatio: 16 / 9 });

  const props: {
    showOverlay: boolean;
    onMuteChange?: (mute: boolean) => void;
  } = $props();

  export function toggleMute(force?: boolean) {
    preview.muted = force != null ? force : !preview.muted;
    isMuted = preview.muted;
    if (props.onMuteChange) props.onMuteChange(isMuted);
  }

  export async function play() {
    return await preview.play();
  }

  export function getStream() {
    return preview.srcObject;
  }

  export function setStream(src: MediaProvider) {
    preview.srcObject = src;
    viewFallbackTransform = {
      rotation: 0,
      zoom: 0,
      aspectRatio: 16 / 9,
    };
  }

  export function setZoom(value: number) {
    viewFallbackTransform.zoom = value;
  }

  export function setRotation(value: number) {
    viewFallbackTransform.rotation = value;
  }

  export function setAspectRatio(value: number) {
    viewFallbackTransform.aspectRatio = value;
  }

  export function getVideo() {
    return preview;
  }
</script>

<div class="watcher">
  <div class="connection-wait">
    <img class="logo" src="/logo.png" alt="branding" />
    <p class="wait">Aguardando Imagem...</p>
  </div>
  {#if props.showOverlay}
    <div class="overlay">
      <button class="muteButton" onclick={() => toggleMute()} aria-label="mute">
        <div class={`muteIcon ${isMuted ? "isMuted" : ""}`}></div>
      </button>
    </div>
  {/if}
  <div
    class="playerContainer"
    style={[
      `--aspect-ratio: ${viewFallbackTransform.aspectRatio};`,
      `--rotate: ${viewFallbackTransform.rotation}deg;`,
      `--scale: ${viewFallbackTransform.rotation % 180 === 90 ? viewFallbackTransform.aspectRatio : "1"};`,
    ].join("")}
  >
    <video
      id="video"
      autoplay
      muted
      playsinline
      bind:this={preview}
      style={`--zoom: ${(viewFallbackTransform.zoom / 100) * 5 + 1};`}
    ></video>
  </div>
</div>

<style>
  .watcher {
    width: 100%;
    height: 100%;
    display: flex;
    position: relative;
    justify-content: center;
    align-items: center;
  }

  .connection-wait {
    position: absolute;
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 1rem;
    justify-content: center;
    align-items: center;
  }

  .logo {
    width: 65%;
    height: auto;
    opacity: 0.75;
  }

  .wait {
    color: var(--accent-color);
    font-size: 1.25rem;
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
    cursor: pointer;
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
    overflow: hidden;
    display: flex;
    place-content: center;
    rotate: var(--rotate);
    overflow: hidden;
    scale: var(--scale);
    width: auto;
    height: 100%;
    max-width: 100%;
    z-index: 10;
  }

  #video {
    width: 100%;
    object-fit: contain;
    scale: var(--zoom);
  }
</style>
