<script lang="ts">
  import QRCode from "qrcode";

  let canvas: HTMLCanvasElement;

  const {
    url,
    size = 256,
    margin = 2,
    dark = "#000000",
    light = "#ffffff"
  } = $props<{
    url: string;
    size?: number;
    margin?: number;
    dark?: string;
    light?: string;
  }>();

  async function renderQR() {
    if (!canvas || !url) return;

    try {
      await QRCode.toCanvas(canvas, url, {
        width: size,
        margin,
        color: { dark, light }
      });
    } catch (err) {
      console.error("QR generation failed:", err);
    }
  }

  $effect(() => {
    renderQR();
  });
</script>

<canvas bind:this={canvas} width={size} height={size}></canvas>