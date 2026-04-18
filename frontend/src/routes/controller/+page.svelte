<div class="controller">
  <nav class="sidebar"></nav>
  <div class="pages">
    <div class="page homepage"></div>
    <div class="page streamers"></div>
  </div>
  <div class="info">
    <div class="preview"></div>
    <div class="logs"></div>
    <img class="logo" alt="Company Branding" src="/image/logo.png" />
  </div>
</div>

<style>
  .controller {
    width: 100%;
    height: 100%;
    display: grid;
    grid-template-columns: 5rem 3fr 1fr;
  }

  .sidebar {
    background: hsla(from var(--accent-color) h s l / 0.3);
    width: 100%;
    height: 100%;
  }

  .info {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding: 1rem;
  }

  .preview {
    width: 100%;
    height: auto;
    aspect-ratio: 16/9;
    border: 1px solid var(--accent-color);
    border-radius: 1rem;
    overflow: clip;
  }

  .logs {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    gap: 0.5rem;
    border: 1px solid var(--accent-color);
    background: var(--black);
    color: var(--accent-color);
  }

  .logo {
    width: 100%;
    object-fit: contain;
  }
</style>
