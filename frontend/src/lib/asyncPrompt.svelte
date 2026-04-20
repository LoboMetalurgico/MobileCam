<script lang="ts">
  import type { Snippet } from "svelte";
  type ShowCase = [Snippet<[unknown[]]>, unknown[]];

  // Internal states for our solitary hunter
  let visible = $state<boolean>(false);
  let result = $state<ShowCase | null>(null);
  let error = $state<string>("");
  let loading = $state<boolean>(false);
  let cancelled = $state<boolean>(false);
  let closeOnClickOutside = $state<boolean>(true);

  // Keep track of the current scent so we don't fetch stale prey
  let currentPromise: Promise<ShowCase> | null = null;

  // The alpha commands the modal to appear and begin the hunt
  export function show(newPromise: Promise<ShowCase>) {
    visible = true;
    loading = true;
    error = "";
    result = null;
    cancelled = false;
    currentPromise = newPromise;

    newPromise
      .then((res) => {
        // Only feast if this is still the alpha promise and we haven't abandoned the hunt
        if (currentPromise === newPromise && !cancelled) {
          result = res;
          loading = false;
        }
      })
      .catch((err) => {
        if (currentPromise === newPromise && !cancelled) {
          error = err.message || "An error occurred";
          loading = false;
        }
      });
  }

  // A swift command to retreat into the shadows
  export function hide() {
    visible = false;
    cancelled = true;
    currentPromise = null;
  }

  // Handling disturbances in the den
  function handleCancel(reason: string) {
    if (closeOnClickOutside) {
      if (loading && !cancelled) {
        // Interrupt the hunt with an error
        error = reason;
        loading = false;
        cancelled = true;
      } else if (!loading) {
        // If the hunt is already over (success or error), clicking outside just dismisses it
        hide();
      }
    }
  }

  function handleClickOutside(event: MouseEvent) {
    if (event.target === event.currentTarget) {
      handleCancel("Promise was cancelled by clicking outside");
    }
  }

  function handleKeyup(event: KeyboardEvent) {
    if (event.key === "Escape") {
      handleCancel("Promise was cancelled by pressing Escape");
    }
  }

  export function updateCloseOnClickOutside(value: boolean) {
    closeOnClickOutside = value;
  }
</script>

{#if visible}
  <div
    class="overlay"
    tabindex="0"
    role="dialog"
    onclick={handleClickOutside}
    onkeyup={handleKeyup}
  >
    <div class={`modal ${loading ? "isLoading" : ""}`}>
      {#if closeOnClickOutside}
        <button
          class="closeButton"
          aria-label="Close"
          onclick={() =>
            handleCancel("Promise was cancelled by clicking the close button")}
          >X</button
        >
      {/if}
      {#if loading}
        <div class="spinner"></div>
      {:else if error}
        <p class="error-text">{error}</p>
      {:else if result}
        <div>{@render result[0](result[1])}</div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background: hsla(from var(--accent-color) h s calc(l - 75) / 0.3);
    display: flex;
    justify-content: center;
    align-items: center;
    z-index: 1000;
  }
  .modal {
    background: hsla(from var(--accent-color) h s calc(l - 50) / 0.3);
    backdrop-filter: blur(4px);
    border: 1px solid var(--accent-color);
    padding: 1.5rem;
    border-radius: 1rem;
    max-width: 80%;
    max-height: 80%;
    overflow-y: auto;
    position: relative;
    interpolate-size: allow-keywords;
    width: fit-content;
    height: fit-content;
    overflow-x: hidden;
    transition:
      width 0.5s ease,
      height 0.5s ease;
  }
  .modal.isLoading {
    width: 15rem;
    height: 15rem;
    overflow: hidden;
  }
  .error-text {
    color: #d32f2f;
    font-weight: bold;
  }
  .closeButton {
    position: absolute;
    top: 0.5rem;
    right: 0.5rem;
    background: transparent;
    border: none;
    color: var(--accent-color);
    font-size: 1rem;
    font-weight: bolder;
    cursor: pointer;
  }
  .spinner {
    border: 1rem solid #fffafa3d;
    border-top: 1rem solid var(--accent-color);
    border-radius: 50%;
    width: 12rem;
    height: 12rem;
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    0% {
      transform: rotate(0deg);
    }
    100% {
      transform: rotate(360deg);
    }
  }
</style>
