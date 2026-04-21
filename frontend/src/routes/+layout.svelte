<script lang="ts">
  import { page } from "$app/state";
  import favicon from "$lib/assets/favicon.svg";

  let { children } = $props();

  $effect(() => {
    fetch(`https://${page.url.host}/color.txt`)
      .then((data) => data.text())
      .then((color) => {
        if (color.startsWith("#"))
          document.body.style.setProperty("--accent-color", color);
      })
      .catch(() => {});
  });
</script>

<svelte:head>
  <link rel="icon" href={favicon} />
</svelte:head>

{@render children()}

<style global>
  @import "$lib/global.css";
</style>
