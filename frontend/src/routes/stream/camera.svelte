<script lang="ts">
  import type { Quality } from "$lib/interfaces/Quality";
  import { preventScreenLock } from "$lib/utils/phoneUtils";

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

  export async function startCamera() {
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

      // @ts-expect-error chrome only feature
      if (caps.zoom) {
        nativeZoomSupported = true;
        // @ts-expect-error chrome only feature
        minZoom = caps.zoom.min;
        // @ts-expect-error chrome only feature
        maxZoom = caps.zoom.max;
      }
    } catch (err) {
      console.error(err);
      alert("Could not start camera");
    }
  }

  export function getVideoStream(): MediaStream | undefined {
    return localStream;
  }
</script>

<div class="videoPreview">
  <div class="hoz-rulers"></div>
  <div class="ver-rulers"></div>
  <video
    id="preview"
    ontouchmove={handleTouchMove}
    ontouchstart={handleTouchStart}
    srcobject={localStream}
    style={`transform: scale(${previewStyleTransform});`}
    autoplay
    muted
    playsinline
  ></video>
</div>

<style>
  .videoPreview {
    width: 100vw;
    max-width: calc(100dvh * (16 / 9) - 1rem);
    aspect-ratio: 16/9;
    height: auto;
    background: black;
    position: relative;
    pointer-events: none;
  }

  #preview {
    width: 100%;
    height: 100%;
    object-fit: contain;
    pointer-events: all;
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
</style>
