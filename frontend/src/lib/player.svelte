<script lang="ts">
  interface Props {
    transform: ({
      zoom,
      rotation,
    }: {
      zoom: number;
      rotation: number;
    }) => void;
    setStreamingBandwidth: (maxBitrate: number, maxFramerate: number) => void;
    
  }

  let props: Props = $props();

  let preview: HTMLVideoElement;
</script>

<video id="video" autoplay playsinline bind:this={preview}></video>