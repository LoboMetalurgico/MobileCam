<script lang="ts">
  interface Props {
    min?: number;
    max?: number;
    step?: number;
    value?: number;
    showValue?: boolean;
    onChange?: (value: number) => void;
  }
  let {
    min = 0,
    max = 100,
    step = 1,
    value = $bindable(0),
    showValue = true,
    onChange,
  }: Props = $props();

  let slider: HTMLInputElement;

  function updateBackground() {
    const ratio = ((value - min) / (max - min)) * 100;
    slider.style.background = `linear-gradient(
      0deg,
      var(--slider-bg-color) ${ratio}%,
      var(--deselected-bg-color) ${ratio}%
    )`;
  }

  $effect(() => {
    if (slider) updateBackground();
  });

  function handleInput(e: Event) {
    const target = e.target as HTMLInputElement;
    value = Number(target.value);
    onChange?.(value);
  }
</script>

<div class="wrapper">
  <div class="labels">
    {#if showValue}
      <span>{max}%</span>
      {#if value !== min && value !== max}
        <span class="current">{Math.round(value)}%</span>
      {/if}
      <span>{min}%</span>
    {/if}
  </div>

  <input
    bind:this={slider}
    class="slider"
    type="range"
    {min}
    {max}
    {step}
    bind:value
    oninput={handleInput}
  />
</div>

<style>
  .wrapper {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    height: 100%;
    width: 100%;
  }

  .labels {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    height: 100%;
    width: fit-content;
    font-size: 0.75rem;
    color: #aaa;
  }

  .current {
    color: var(--accent-color);
    font-weight: bold;
  }

  .slider {
    --slider-bg-color: hsla(from var(--accent-color) h calc(s / 3) l);
    --deselected-bg-color: #444;

    appearance: none;
    width: 100%;
    height: 100%;
    cursor: pointer;

    writing-mode: vertical-rl;
    direction: rtl;

    background: linear-gradient(
      0deg,
      var(--slider-bg-color) 0%,
      var(--deselected-bg-color) 0%
    );
  }

  /* Chrome, Edge, Safari */
  .slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 100%;
    height: 0.4rem;
    background: var(--accent-color);
    border: none;
    border-radius: 0;
  }

  /* Firefox */
  .slider::-moz-range-thumb {
    width: 100%;
    height: 0.4rem;
    background: var(--accent-color);
    border: none;
    border-radius: 0;
  }
</style>
