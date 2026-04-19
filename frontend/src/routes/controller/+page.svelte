<div class="controller">
  <nav class="sidebar"></nav>
  <div class="pages">
    <div class="page homepage"></div>
    <div class="page streamers"></div>
  </div>
  <div class="info">
    <div class="preview"></div>
    <div class="logs">
      <h2 class="logsBoxTitle">Logs</h2>
      <p class="logText">
        &gt; MobileCam V1.0.0
        <br />
        <br />
        Transmissores: 0 <br />
        Receptores: 0
        <br />
        <br />
        Controller_id: 0
      </p>
    </div>
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

  .logsBoxTitle {
    font-size: 1.25rem;
    padding: 0.5rem 1rem;
    background: hsla(from var(--accent-color) h s l / 0.3);
    width: 100%;
    height: fit-content;
  }

  .logText {
    padding-inline: .5rem;
    font-family: monospace;
    font-size: 1rem;
  }

  .logo {
    width: 100%;
    object-fit: contain;
  }
</style>
