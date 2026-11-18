<script lang="ts">
  import type { InputUnit } from '@nodarium/types';
  import {
    formatStep,
    formatValue,
    parseValue,
    roundDisplay,
    unitFactor,
    unitLabel as labelOf,
    unitSettings
  } from '../units.svelte';

  interface Props {
    value?: number;
    step?: number;
    min?: number;
    max?: number;
    id?: string;
    /** Marks the value as a length in meters or an angle in radians, shown in the unit of the settings */
    unit?: InputUnit;
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

  const factor = $derived(unitFactor(unit));
  const unitLabel = $derived(unitSettings.show ? labelOf(unit) : undefined);

  // snaps in shown units, so a step of 0.01 rad drags by whole degrees
  function snap(v: number, s = step) {
    let shown = v * factor;
    if (s) {
      const shownStep = formatStep(s, unit);
      shown = Math.round(shown / shownStep) * shownStep;
    }
    shown = +shown.toFixed(3);
    return factor === 1 ? shown : roundDisplay(shown / factor);
  }

  // the auto step follows the magnitude of the shown value, so 25 cm snaps to 10 cm and not to 0.1 m
  function getAutoStep(v: number): number {
    const abs = Math.abs(v * factor);
    if (abs === 0) return 0.1 / factor; // fallback for 0
    const exponent = Math.floor(Math.log10(abs));
    return Math.pow(10, exponent) / factor;
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

  const displayValue = $derived(formatValue(value, unit));

  const width = $derived(
    Number.isFinite(value)
      ? `${Math.max((displayValue.toString().length + (unitLabel ? 3 : 0)) * 8, 50)}px`
      : '20px'
  );

  // the input is text, so "10cm", "45°", "pi / 2" or "2 * 3" can be typed, a plain number is read in the shown unit
  function onChange(e: Event & { currentTarget: HTMLInputElement }) {
    const parsed = parseValue(e.currentTarget.value, unit);
    if (parsed !== undefined) {
      value = parsed;
    }
    // show the parsed value in the shown unit again, also when the text was invalid
    e.currentTarget.value = String(formatValue(value, unit));
  }

  function stepBy(e: MouseEvent, direction: number) {
    value = snap(value + direction * (e.shiftKey ? 10 : 1) * formatStep(step!, unit) / factor);
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
      onmousedown={(e) => stepBy(e, -1)}
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
      value={displayValue}
      onchange={onChange}
      type="text"
      {id}
      step={step ? formatStep(step, unit) : step}
      min={formatValue(min, unit)}
      max={formatValue(max, unit)}
      inputmode="decimal"
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
      onmousedown={(e) => stepBy(e, 1)}
      class="cursor-pointer w-4 bg-layer-3/30 hover:bg-layer-3/50"
    >
      <span class="i-[tabler--chevron-right] block h-full w-full text-outline!"></span>
    </button>
  {/if}
</div>

<style>
  input {
    -webkit-appearance: textfield;
    -moz-appearance: textfield;
    appearance: textfield;
  }
  input::-webkit-inner-spin-button,
  input::-webkit-outer-spin-button {
    -webkit-appearance: none;
  }
</style>
