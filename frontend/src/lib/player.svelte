<script lang="ts">
  let isMuted = $state(true);
  let preview: HTMLVideoElement;
  let viewFallbackTransform = $state<{
    rotation: number;
    zoom: number;
    aspectRatio: number;
    isNative: boolean;
  }>({ rotation: 0, zoom: 1, aspectRatio: 16 / 9, isNative: true });

  const props: { showOverlay: boolean } = $props();

  function toggleMute() {
    preview.muted = !preview.muted;
    isMuted = preview.muted;
  }

  export async function play() {
    return await preview.play();
  }

  export function getStream() {
    return preview.srcObject;
  }

  export function setStream(src: MediaProvider) {
    preview.srcObject = src;
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

  export function setIsNative(bool: boolean) {
    viewFallbackTransform.isNative = bool;
  }

  export function getVideo() {
    return preview;
  }
</script>

<div class="watcher">
  {#if props.showOverlay}
    <div class="overlay">
      <button class="muteButton" onclick={toggleMute} aria-label="mute">
        <div class={`muteIcon ${isMuted ? "isMuted" : ""}`}></div>
      </button>
    </div>
  {/if}
  <div
    class="playerContainer"
    style={[
      `--aspect-ratio: ${!viewFallbackTransform.isNative && viewFallbackTransform.rotation % 180 === 0 ? 1 / viewFallbackTransform.aspectRatio : viewFallbackTransform.aspectRatio};`,
      `--rotate: ${viewFallbackTransform.rotation}deg;`,
      viewFallbackTransform.isNative
        ? "width: 100dvw;"
        : `width: calc(${viewFallbackTransform.rotation % 180 === 0 ? "100dvh /" : "100dvw *"} var(--aspect-ratio));`,
      viewFallbackTransform.isNative
        ? "height: 100dvh;"
        : `height: ${viewFallbackTransform.rotation % 180 === 0 ? "100dvh" : "100dvw"};`,
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
    aspect-ratio: var(--aspect-ratio);
  }

  #video {
    width: 100%;
    object-fit: contain;
    background-color: #111;
    scale: var(--zoom);
  }
</style>
