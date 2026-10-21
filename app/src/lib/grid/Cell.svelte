<script lang="ts">
  import type { Snippet } from 'svelte';
  import { getGridRow } from './context';

  const row = getGridRow();
  const index = row.registerCell();
  const sizes = row.sizes;
  let wrapper: HTMLDivElement;

  const { children }: { children?: Snippet } = $props();

  let downWidth = 0;
  let mouseDown = false;
  let startX = 0;

  function handleMouseDown(event: MouseEvent) {
    mouseDown = true;
    startX = event.clientX;
    downWidth = wrapper.getBoundingClientRect().width;
  }

  function handleMouseMove(event: MouseEvent) {
    if (mouseDown) {
      const width = downWidth + startX - event.clientX;
      sizes.value[index] = `${width}px`;
    }
  }
</script>

<svelte:window
  onmouseup={() => (mouseDown = false)}
  onmousemove={handleMouseMove}
/>

{#if index > 0}
  <div
    class="seperator"
    role="button"
    tabindex="0"
    onmousedown={handleMouseDown}
  >
  </div>
{/if}

<div class="cell" bind:this={wrapper}>
  {@render children?.()}
</div>

<style>
  .cell {
    display: block;
    position: relative;
    overflow: hidden;
  }
  .seperator {
    position: relative;
    cursor: ew-resize;
    height: 100%;
    width: 1px;
    background: var(--color-outline);
  }
  .seperator::before {
    content: "";
    cursor: ew-resize;
    position: absolute;
    pointer-events: all;
    height: 100%;
    width: 14px;
    z-index: 2;
    left: -7px;
  }
</style>
