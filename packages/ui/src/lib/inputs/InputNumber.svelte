<script lang="ts">
  import { onMount } from 'svelte';

  interface Props {
    value?: number;
    step?: number;
    min?: number;
    max?: number;
    id?: string;
  }

  let {
    value = $bindable(0.5),
    step,
    min = $bindable(0),
    max = $bindable(1),
    id
  }: Props = $props();

  // normalize bounds
  if (min > max) [min, max] = [max, min];

  let inputEl: HTMLInputElement | undefined = $state();

  function clamp(v: number) {
    return Math.min(max, Math.max(min, v));
  }

  function snap(v: number) {
    if (step) v = Math.round(v / step) * step;
    return +v.toFixed(3);
  }

  let dragging = $state(false);
  let startValue = 0;
  let rect: DOMRect;

  function onMouseDown(e: MouseEvent) {
    if (!inputEl) return;

    e.preventDefault();

    dragging = true;
    startValue = value;
    rect = inputEl.getBoundingClientRect();

    document.body.style.cursor = 'ew-resize';
    window.addEventListener('mousemove', onMouseMove);
    window.addEventListener('mouseup', onMouseUp);
  }

  function onMouseMove(e: MouseEvent) {
    const ratio = (e.clientX - rect.left) / rect.width;
    let delta = (min + (max - min) * ratio) - startValue;

    if (e.shiftKey) delta /= 5;

    value = snap(
      e.ctrlKey
        ? startValue + delta
        : clamp(startValue + delta)
    );
  }

  function onMouseUp() {
    dragging = false;
    document.body.style.cursor = '';
    window.removeEventListener('mousemove', onMouseMove);
    window.removeEventListener('mouseup', onMouseUp);
    if (startValue === value) {
      inputEl?.focus();
    }

    min = Math.min(min, value);
    max = Math.max(max, value);
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape' || e.key === 'Enter') {
      onMouseUp();
      inputEl?.blur();
    }
  }

  function windowKeyDown(e: KeyboardEvent) {
    if (!dragging) return;
    if (e.shiftKey) {
      startValue = value;
    }
  }

  const width = $derived(
    Number.isFinite(value)
      ? `${Math.max(value.toString().length * 8, 50)}px`
      : '20px'
  );

  function stepUp(e: MouseEvent) {
    value = snap(value + (e.shiftKey ? step! * 10 : step!));
  }

  function stepDown(e: MouseEvent) {
    value = snap(value - (e.shiftKey ? step! * 10 : step!));
  }

  onMount(() => {
    value = snap(value);
  });
</script>

<svelte:window onkeydown={windowKeyDown} />

<div
  class="component-wrapper relative flex items-stretch overflow-hidden rounded-sm border border-outline bg-layer-2 select-none transition-shadow"
  class:cursor-ew-resize={dragging}
>
  {#if step}
    <button
      aria-label="step down"
      onmousedown={stepDown}
      class="cursor-pointer w-4 bg-layer-3 opacity-30 hover:opacity-50"
    >
      <span class="i-[tabler--chevron-compact-left] block h-full w-full text-outline!"></span>
    </button>
    <div class="w-px bg-outline"></div>
  {/if}

  <div class="relative grow">
    <div
      class="absolute inset-y-0 left-0 bg-layer-3 opacity-30 pointer-events-none transition-[width]"
      class:transition-none={dragging}
      style={`width: ${Math.min((value - min) / (max - min), 1) * 100}%`}
    >
    </div>

    <input
      bind:this={inputEl}
      bind:value
      {id}
      {step}
      {min}
      {max}
      type="number"
      onkeydown={onKeyDown}
      onmousedown={onMouseDown}
      class="w-full min-w-full cursor-pointer bg-transparent px-2 py-1 text-center font-tabular text-text outline-none appearance-none"
      style:width={width}
    />
  </div>

  {#if step}
    <div class="w-px bg-outline"></div>
    <button
      aria-label="step up"
      onmousedown={stepUp}
      class="cursor-pointer w-4 bg-layer-3 opacity-30 hover:opacity-50"
    >
      <span class="i-[tabler--chevron-compact-right] block h-full w-full text-outline!"></span>
    </button>
  {/if}
</div>

<style>
  input[type="number"] {
    -webkit-appearance: textfield;
    -moz-appearance: textfield;
    appearance: textfield;
  }
  input[type="number"]::-webkit-inner-spin-button,
  input[type="number"]::-webkit-outer-spin-button {
    -webkit-appearance: none;
  }
</style>
