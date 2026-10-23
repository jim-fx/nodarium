<script lang="ts">
  import { roundDisplay, UNIT_FACTORS, unitSettings } from '../units.svelte';

  interface Props {
    value?: number;
    step?: number;
    min?: number;
    max?: number;
    id?: string;
    /** Marks the value as a length in meters, it is shown in the project unit */
    unit?: 'm';
  }

  let {
    value = $bindable(0.5),
    step,
    min = $bindable(0),
    max = $bindable(1),
    id,
    unit
  }: Props = $props();

  // normalize bounds
  if (min > max) [min, max] = [max, min];

  let inputEl: HTMLInputElement | undefined = $state();

  function clamp(v: number) {
    return Math.min(max, Math.max(min, v));
  }

  // the value is stored in meters, the input shows it in the project unit
  const factor = $derived(unit ? UNIT_FACTORS[unitSettings.unit] : 1);
  const unitLabel = $derived(unit && unitSettings.show ? unitSettings.unit : undefined);

  function snap(v: number, s = step) {
    if (s) v = Math.round(v / s) * s;
    return +v.toFixed(3 + Math.round(Math.log10(factor)));
  }

  function getAutoStep(v: number): number {
    const abs = Math.abs(v);
    if (abs === 0) return 0.1; // fallback for 0
    const exponent = Math.floor(Math.log10(abs));
    return Math.pow(10, exponent);
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
        : clamp(startValue + delta),
      (e.altKey && !step) ? getAutoStep(value) : step
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

  const displayValue = $derived(roundDisplay(value * factor));

  const width = $derived(
    Number.isFinite(value)
      ? `${Math.max((displayValue.toString().length + (unitLabel ? 3 : 0)) * 8, 50)}px`
      : '20px'
  );

  function stepUp(e: MouseEvent) {
    value = snap(value + (e.shiftKey ? step! * 10 : step!));
  }

  function stepDown(e: MouseEvent) {
    value = snap(value - (e.shiftKey ? step! * 10 : step!));
  }
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
      class="cursor-pointer w-4 bg-layer-3/30 hover:bg-layer-3/50"
    >
      <span class="i-[tabler--chevron-left] block h-full w-full text-outline!"></span>
    </button>
    <div class="w-px bg-outline"></div>
  {/if}

  <div class="relative grow">
    <div
      class="absolute inset-y-0 left-0 bg-layer-3 opacity-30 pointer-events-none transition-[width]"
      class:transition-none={dragging}
      style={`width: ${Math.max(0, Math.min((value - min) / (max - min), 1) * 100)}%`}
    >
    </div>

    <input
      bind:this={inputEl}
      bind:value={() => displayValue, (v) => (value = v / factor)}
      {id}
      step={step ? step * factor : step}
      min={min * factor}
      max={max * factor}
      type="number"
      onkeydown={onKeyDown}
      onmousedown={onMouseDown}
      class="w-full min-w-full cursor-pointer bg-transparent px-2 py-1 text-center font-tabular text-text outline-none appearance-none"
      style:width={width}
    />
    {#if unitLabel}
      <span
        class="absolute inset-y-0 right-1.5 flex items-center text-xs text-text/50 pointer-events-none"
        data-testid="unit-label"
      >
        {unitLabel}
      </span>
    {/if}
  </div>

  {#if step}
    <div class="w-px bg-outline"></div>
    <button
      aria-label="step up"
      onmousedown={stepUp}
      class="cursor-pointer w-4 bg-layer-3/30 hover:bg-layer-3/50"
    >
      <span class="i-[tabler--chevron-right] block h-full w-full text-outline!"></span>
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
