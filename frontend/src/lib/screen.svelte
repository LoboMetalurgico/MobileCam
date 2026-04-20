<script lang="ts">
  import { preventScreenLock } from "$lib/utils/phoneUtils";
  import { Quality } from "./protos/common";
  const QUALITY_PROFILES = {
    [Quality.UNRECOGNIZED]: {
      frameRate: 5,
      bitrate: 300_000,
    },
    [Quality.QUALITY_UNSPECIFIED]: {
      frameRate: 5,
      bitrate: 300_000,
    },
    [Quality.QUALITY_HIGH]: {
      frameRate: 30,
      bitrate: 8_000_000,
    },
    [Quality.QUALITY_MEDIUM]: {
      frameRate: 15,
      bitrate: 1_000_000,
    },
    [Quality.QUALITY_LOW]: {
      frameRate: 5,
      bitrate: 300_000,
    },
  };

  let currentQuality: Quality = Quality.QUALITY_LOW;

  let videoWidth = $state(0);
  let videoHeight = $state(0);

  let localStream: MediaStream | undefined = $state();

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
            frameRate: q.frameRate,
          })
          .catch(() => {});
      }
    }

    await props.setStreamingBandwidth(q.bitrate, q.frameRate);
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
      localStream = await navigator.mediaDevices.getDisplayMedia({
        video: {
          frameRate: q.frameRate,
        },
        audio: true,
      });

      const track = localStream.getVideoTracks()[0];
      const settings = track.getSettings();

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
      alert("Could not start video capture");
    }
  }

  export function getVideoStream(): MediaStream | undefined {
    return localStream;
  }
</script>

<div class="VideoContent">
  <div
    class="videoPreview"
    style={`--width: ${videoWidth};--height: ${videoHeight};`}
  >
    <video id="preview" srcobject={localStream} autoplay muted playsinline
    ></video>
  </div>
</div>

<style>
  .VideoContent {
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
</style>
