<script lang="ts">
  import { Quality } from "$lib/protos/common";
  import { preventScreenLock } from "$lib/utils/phoneUtils";
  import Player from "./player.svelte";
  import VerticalSlider from "./VerticalSlider.svelte";
  const QUALITY_PROFILES = {
    [Quality.UNRECOGNIZED]: {
      width: 1920,
      height: 1080,
      frameRate: 30,
      bitrate: 8_000_000,
    },
    [Quality.QUALITY_UNSPECIFIED]: {
      width: 1920,
      height: 1080,
      frameRate: 30,
      bitrate: 8_000_000,
    },
    [Quality.QUALITY_HIGH]: {
      width: 1920,
      height: 1080,
      frameRate: 30,
      bitrate: 8_000_000,
    },
    [Quality.QUALITY_MEDIUM]: {
      width: 1280,
      height: 720,
      frameRate: 24,
      bitrate: 2_000_000,
    },
    [Quality.QUALITY_LOW]: {
      width: 640,
      height: 480,
      frameRate: 15,
      bitrate: 500_000,
    },
  };

  let currentQuality: Quality = Quality.QUALITY_HIGH;

  let zoomValue = $state(0);
  let maxZoom = $state(5);
  let minZoom = $state(1);
  let nativeZoomSupported = false;
  const facingMode = "environment";
  let initialPinchDistance = 0;
  let pinchStartZoom = 1;
  let previewStyleTransform = $state({
    zoom: 0,
    rotation: 0,
  });

  let localStream: MediaStream | undefined = $state();
  let preview: Player;

  interface Props {
    onTransform: ({
      zoom,
      rotation,
      isNative,
    }: {
      zoom: number;
      rotation: number;
      isNative: boolean;
    }) => void;
    setStreamingBandwidth: (
      maxBitrate: number,
      maxFramerate: number,
    ) => Promise<void>;
  }

  let props: Props = $props();

  export function applyZoom(val: number) {
    if (!localStream) return;
    // convert val from 0 to 100 into min to max
    const appliedZoom = (val / 100) * (maxZoom - minZoom) + minZoom;
    zoomValue = appliedZoom;
    const track = localStream.getVideoTracks()[0];

    if (nativeZoomSupported) {
      previewStyleTransform.zoom = 0;
      track
        .applyConstraints({
          advanced: [{ zoom: appliedZoom } as MediaTrackConstraintSet],
        })
        .catch(() => {
          nativeZoomSupported = false;
          previewStyleTransform.zoom = val;
        });
    }

    if (!nativeZoomSupported) {
      previewStyleTransform.zoom = val;
      props.onTransform({ ...previewStyleTransform, isNative: false });
      preview.setZoom(val);
    } else {
      props.onTransform({ rotation: 0, zoom: val, isNative: true });
      preview.setZoom(0);
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
        vTrack.contentHint = "detail";
        await vTrack
          .applyConstraints({
            width: { exact: q.width, min: q.width, max: q.width },
            height: { exact: q.height, min: q.height, max: q.height },
            frameRate: {
              exact: q.frameRate,
              min: q.frameRate,
              max: q.frameRate,
            },
          })
          .catch(() => {});
      }
    }

    await props.setStreamingBandwidth(q.bitrate, q.frameRate);
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
    const rawZoom = pinchStartZoom * scale;
    const clampedZoom = Math.max(minZoom, Math.min(maxZoom, rawZoom));
    const percent = ((clampedZoom - minZoom) / (maxZoom - minZoom)) * 100;
    applyZoom(percent);
  }

  function handleVideoResize() {
    setTimeout(() => {
      if (!localStream) return;
      const settings = localStream.getVideoTracks()[0].getSettings();
      const videoRatio = settings.aspectRatio ?? 16 / 9;
      if (!nativeZoomSupported) preview.setAspectRatio(videoRatio);
    }, 500);
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

      preview.setStream(localStream);
      await preview.play();

      // @ts-expect-error chrome only feature
      if (caps.zoom) {
        nativeZoomSupported = true;
        // @ts-expect-error chrome only feature
        minZoom = caps.zoom.min;
        // @ts-expect-error chrome only feature
        maxZoom = caps.zoom.max;
      }

      applyZoom(0);

      screen.orientation.addEventListener("change", () => {
        handleVideoResize();
        if (nativeZoomSupported) {
          preview.setRotation(0);
          return;
        }
        const angle = screen.orientation.type.includes("landscape-primary")
          ? 270
          : screen.orientation.type.includes("landscape-secondary")
            ? 90
            : screen.orientation.type.includes("portrait-primary")
              ? 0
              : 180;
        if (previewStyleTransform.rotation === angle) return;
        previewStyleTransform.rotation = angle;
        preview.setRotation(angle);
        props.onTransform({ ...previewStyleTransform, isNative: false });
      });
    } catch (err) {
      console.error(err);
      alert("Could not start camera");
    }
  }

  export function getVideoStream(): MediaStream | undefined {
    return localStream;
  }
</script>

<div class="CameraContent">
  <div class="overlay">
    <VerticalSlider
      min={0}
      max={100}
      step={1}
      value={((zoomValue - minZoom) / (maxZoom - minZoom)) * 100}
      showValue={false}
      onChange={(newValue) => {
        applyZoom(newValue);
      }}
    />
  </div>
  <div
    class="videoPreview"
    role="img"
    ontouchmove={handleTouchMove}
    ontouchstart={handleTouchStart}
  >
    <div class="hoz-rulers"></div>
    <div class="ver-rulers"></div>
    <Player bind:this={preview} showOverlay={false} />
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
  .overlay {
    position: absolute;
    width: 7rem;
    height: 100%;
    top: 0;
    right: 0;
    padding: 1rem;
    z-index: 999;
  }

  .videoPreview {
    background: black;
    position: relative;
    pointer-events: none;
    overflow: hidden;
    width: 100%;
    height: 100%;
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
