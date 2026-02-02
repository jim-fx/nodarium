<script lang="ts">
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

  if (min > max) {
    [min, max] = [max, min];
  }
  if (value > max) {
    max = value;
  }

  const precision = $derived(
    ((step || value).toString().split('.')[1] || '').length
  );

  function strip(input: number) {
    return +parseFloat(input + '').toFixed(precision);
  }

  let inputEl: HTMLInputElement | undefined = $state();

  let oldValue: number;
  function handleChange() {
    if (value === oldValue) return;
    oldValue = value;
  }

  let isMouseDown = $state(false);
  let downV = 0;
  let vx = 0;
  let rect: DOMRect;

  function handleMouseDown(ev: MouseEvent) {
    ev.preventDefault();

    if (!inputEl) return;

    inputEl.focus();

    isMouseDown = true;

    downV = value;
    rect = inputEl.getBoundingClientRect();

    window.removeEventListener('mousemove', handleMouseMove);
    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);
    document.body.style.cursor = 'ew-resize';
    (ev.target as HTMLElement)?.blur();
  }

  function handleMouseUp() {
    isMouseDown = false;

    if (downV === value) {
      inputEl?.focus();
    }

    if (value > max) {
      max = value;
    }

    if (value < min) {
      min = value;
    }

    document.body.style.cursor = 'unset';
    window.removeEventListener('mouseup', handleMouseUp);
    window.removeEventListener('mousemove', handleMouseMove);
  }

  function handleKeyDown(ev: KeyboardEvent) {
    if (ev.key === 'Escape' || ev.key === 'Enter') {
      handleMouseUp();
      inputEl?.blur();
    }
  }

  function handleMouseMove(ev: MouseEvent) {
    vx = (ev.clientX - rect.left) / rect.width;

    if (ev.ctrlKey) {
      let v = min + (max - min) * vx;
      value = v;
    } else {
      value = Math.max(Math.min(min + (max - min) * vx, max), min);
    }

    value = strip(value);

    // With ctrl + outside of input ev.target becomes HTMLDocument
    if (ev.target instanceof HTMLElement) {
      ev.target?.blur();
    }
  }

  $effect(() => {
    if (value.toString().length > 5) {
      value = strip(value);
    }
    if (value !== undefined) {
      handleChange();
    }
  });

  let width = $derived(
    Number.isFinite(value)
      ? Math.max((value?.toString().length ?? 1) * 8, 50) + 'px'
      : '20px'
  );
</script>

<div class="component-wrapper" class:is-down={isMouseDown}>
  <span class="overlay" style={`width: ${((value - min) / (max - min)) * 100}%`}></span>
  <input
    bind:value
    bind:this={inputEl}
    {id}
    {step}
    {max}
    {min}
    onkeydown={handleKeyDown}
    onmousedown={handleMouseDown}
    onmouseup={handleMouseUp}
    type="number"
    style={`width:${width};`}
  />
</div>

<style>
  .component-wrapper {
    position: relative;
    background-color: var(--layer-2, #4b4b4b);
    border-radius: 4px;
    user-select: none;
    transition: box-shadow 0.3s ease;
    border: solid 1px var(--outline);
    box-sizing: border-box;
    overflow: hidden;
    border-radius: var(--border-radius, 2px);
  }

  input[type="number"]::-webkit-inner-spin-button,
  input[type="number"]::-webkit-outer-spin-button {
    -webkit-appearance: none;
  }

  input[type="number"] {
    box-sizing: border-box;
    -webkit-appearance: textfield;
    -moz-appearance: textfield;
    appearance: textfield;
    font-family: var(--font-family);
    font-variant-numeric: tabular-nums;
    cursor: pointer;
    color: var(--text-color);
    background-color: transparent;
    padding: var(--padding, 6px);
    font-size: 1em;
    padding-inline: 10px;
    text-align: center;
    border: none;
    border-style: none;
    min-width: 100%;
  }

  .is-down > input {
    cursor: ew-resize !important;
  }

  .overlay {
    position: absolute;
    top: 0px;
    left: 0px;
    height: 100%;
    max-width: 100%;
    background-color: var(--text-color);
    opacity: 0.3;
    pointer-events: none;
    transition: width 0.3s ease;
  }

  .is-down > .overlay {
    transition: none !important;
  }
</style>
