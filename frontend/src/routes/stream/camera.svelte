<script lang="ts">
  import type { Quality } from "$lib/interfaces/Quality";
  import { preventScreenLock } from "$lib/utils/phoneUtils";
  import { onMount } from "svelte";

  const QUALITY_PROFILES = {
    high: { width: 1920, height: 1080, frameRate: 30, bitrate: 8_000_000 },
    medium: { width: 1280, height: 720, frameRate: 24, bitrate: 2_000_000 },
    low: { width: 640, height: 480, frameRate: 15, bitrate: 500_000 },
  };

  let currentQuality: Quality = "high";

  let zoomValue = 1;
  let maxZoom = 5;
  let minZoom = 1;
  let nativeZoomSupported = false;
  const facingMode = "environment";
  let initialPinchDistance = 0;
  let pinchStartZoom = 1;
  let previewStyleTransform = $state({
    zoom: 1,
    rotation: 0,
  });
  let videoWidth = $state(0);
  let videoHeight = $state(0);
  let zoomSlider: HTMLInputElement;

  let localStream: MediaStream | undefined = $state();

  interface Props {
    onTransform: ({
      zoom,
      rotation,
    }: {
      zoom: number;
      rotation: number;
    }) => void;
    setStreamingBandwidth: (maxBitrate: number, maxFramerate: number) => void;
  }

  let props: Props = $props();

  function applyZoom(val: number) {
    if (!localStream) return;
    zoomValue = val;
    updateZoomSlider();
    const track = localStream.getVideoTracks()[0];

    if (nativeZoomSupported) {
      previewStyleTransform.zoom = 1;
      track
        .applyConstraints({
          advanced: [{ zoom: val } as MediaTrackConstraintSet],
        })
        .catch(() => {
          nativeZoomSupported = false;
          previewStyleTransform.zoom = val;
        });
    }

    if (!nativeZoomSupported) {
      const settings = track.getSettings();
      previewStyleTransform.zoom = val;
      previewStyleTransform.rotation =
        (settings.width ?? 1920) < (settings.height ?? 1080) ? 90 : 0;
      props.onTransform(previewStyleTransform);
    }
  }

  export function getCurrentBandwidth(): {
    maxBitrate: number;
    maxFramerate: number;
  } {
    return {
      maxBitrate: QUALITY_PROFILES[currentQuality].bitrate,
      maxFramerate: QUALITY_PROFILES[currentQuality].frameRate,
    };
  }

  export async function applyQuality(quality: Quality) {
    const q = QUALITY_PROFILES[quality];
    if (!q) return;
    currentQuality = quality;

    if (localStream) {
      const vTrack = localStream.getVideoTracks()[0];
      if (vTrack) {
        await vTrack
          .applyConstraints({
            width: { exact: q.width },
            height: { exact: q.height },
            frameRate: { exact: q.frameRate },
          })
          .catch(() => {});
      }
    }

    props.setStreamingBandwidth(q.bitrate, q.frameRate);
  }

  function handleTouchStart(e: TouchEvent) {
    console.log(e.touches.length);
    if (e.touches.length === 2) {
      initialPinchDistance = Math.hypot(
        e.touches[0].clientX - e.touches[1].clientX,
        e.touches[0].clientY - e.touches[1].clientY,
      );
      pinchStartZoom = zoomValue;
    }
  }

  function handleTouchMove(e: TouchEvent) {
    if (e.touches.length !== 2) return;
    const dist = Math.hypot(
      e.touches[0].clientX - e.touches[1].clientX,
      e.touches[0].clientY - e.touches[1].clientY,
    );
    const scale = dist / initialPinchDistance;
    const newZoom = Math.max(
      minZoom,
      Math.min(maxZoom, pinchStartZoom * scale),
    );
    applyZoom(newZoom);
  }

  export async function recalculateVideoDimensions(retryAttempt = 0) {
    if (!localStream) return;
    const settings = localStream.getVideoTracks()[0].getSettings();
    const previousWidth = videoWidth;
    const previousHeight = videoHeight;
    if (
      settings.width === previousWidth &&
      settings.height === previousHeight
    ) {
      // check again in 100ms in case the settings haven't updated yet
      if (retryAttempt < 5)
        setTimeout(() => recalculateVideoDimensions(retryAttempt + 1), 100);
      return;
    }
    videoWidth = settings.width ?? 0;
    videoHeight = settings.height ?? 0;
  }

  export async function start() {
    try {
      await preventScreenLock();
      const q = QUALITY_PROFILES[currentQuality];
      localStream = await navigator.mediaDevices.getUserMedia({
        video: {
          aspectRatio: { exact: 16 / 9 },
          facingMode: { ideal: facingMode },
          width: { exact: q.width },
          height: { exact: q.height },
          frameRate: { exact: q.frameRate },
        },
        audio: {
          echoCancellation: false,
          noiseSuppression: false,
          autoGainControl: false,
        },
      });

      const track = localStream.getVideoTracks()[0];
      const caps = track.getCapabilities?.() || {};
      const settings = track.getSettings();

      // @ts-expect-error chrome only feature
      if (caps.zoom) {
        nativeZoomSupported = true;
        // @ts-expect-error chrome only feature
        minZoom = caps.zoom.min;
        // @ts-expect-error chrome only feature
        maxZoom = caps.zoom.max;
      }

      applyZoom(1);

      if (settings) {
        videoWidth = settings.width ?? 0;
        videoHeight = settings.height ?? 0;
      }
      screen.orientation.addEventListener("change", () =>
        recalculateVideoDimensions(),
      );
      window.addEventListener("resize", () => recalculateVideoDimensions());
    } catch (err) {
      console.error(err);
      alert("Could not start camera");
    }
  }

  export function getVideoStream(): MediaStream | undefined {
    return localStream;
  }

  function updateZoomSlider() {
    zoomSlider.min = minZoom as unknown as string;
    zoomSlider.max = maxZoom as unknown as string;
    zoomSlider.value = zoomValue as unknown as string;
    const ratio =
      ((Number(zoomSlider.value) - Number(zoomSlider.min)) /
        (Number(zoomSlider.max) - Number(zoomSlider.min))) *
      100;
    zoomSlider.style.background = `linear-gradient(0deg, var(--slider-bg-color) ${ratio}%, var(--deselected-bg-color) ${ratio}%)`;
  }

  onMount(() => {
    zoomSlider.addEventListener("input", () => {
      applyZoom(Number(zoomSlider.value));
    });
  });
</script>

<div class="CameraContent">
  <div class="zoomSlider">
    <input
      type="range"
      name="range"
      value="0"
      min="0"
      max="100"
      step="0.01"
      id="inputRange"
      class="inputRange clickable"
      bind:this={zoomSlider}
    />
  </div>
  <div
    class="videoPreview"
    style={`--width: ${videoWidth};--height: ${videoHeight};`}
  >
    <div class="hoz-rulers"></div>
    <div class="ver-rulers"></div>
    <video
      id="preview"
      ontouchmove={handleTouchMove}
      ontouchstart={handleTouchStart}
      srcobject={localStream}
      style={`transform: rotate(${previewStyleTransform.rotation}) scale(${previewStyleTransform.zoom});`}
      autoplay
      muted
      playsinline
    ></video>
  </div>
</div>

<style>
  .CameraContent {
    width: 100%;
    height: 100%;
    display: flex;
    justify-content: center;
    align-items: center;
  }
  .videoPreview {
    width: 100vw;
    max-width: calc(90dvh * (var(--width) / var(--height)) - 1rem);
    aspect-ratio: calc(var(--width) / var(--height));
    height: auto;
    background: black;
    position: relative;
    pointer-events: none;
    overflow: hidden;
  }

  #preview {
    width: 100%;
    height: 100%;
    object-fit: contain;
    pointer-events: all;
    position: absolute;
    top: 0;
    left: 0;
  }

  .hoz-rulers,
  .ver-rulers {
    position: absolute;
    width: 100%;
    height: 100%;
    top: 0;
    left: 0;
    display: flex;
    flex-direction: row;
    justify-content: space-evenly;
    align-items: center;
    z-index: 99;

    &::before,
    &::after {
      content: "";
      width: 1px;
      height: 100%;
      backdrop-filter: invert();
    }
  }

  .ver-rulers {
    flex-direction: column;

    &::before,
    &::after {
      content: "";
      width: 100%;
      height: 1px;
    }
  }

  .zoomSlider {
    position: absolute;
    right: 0;
    top: 0;
    z-index: 999;
    height: 100%;
    padding: 1rem;
  }

  .inputRange {
    --slider-bg-color: hsla(from var(--accent-color) h calc(s/3) l);
    --deselected-bg-color: #444;

    appearance: none;
    width: 3rem;
    height: 100%;
    border: 1px solid #333333;
    background: linear-gradient(
      0deg,
      var(--slider-bg-color) 0%,
      var(--deselected-bg-color) 0%
    );
    writing-mode: vertical-rl;
    direction: rtl;
    cursor: pointer;
  }

  /* Thumb: for Chrome, Safari, Edge */
  .inputRange::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 3rem;
    height: 0.5rem;
    background: var(--accent-color);
    box-shadow: none;
  }

  /* Thumb: for Firefox */
  .inputRange::-moz-range-thumb {
    border: none;
    border-radius: 0;
    width: 3rem;
    height: 0.5rem;
    background: var(--accent-color);
    box-shadow: none;
  }
</style>
