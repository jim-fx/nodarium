<script lang="ts">
  import { getGraphState } from '../graph-state.svelte';

  const { safePadding }: {
    safePadding?: { left?: number; right?: number; bottom?: number; top?: number };
  } = $props();

  const graphState = getGraphState();
</script>

<div class="zoom-indicator" style:right="calc({safePadding?.right ?? 0}px + 10px)">
  <button
    class="fit-btn"
    title="Fit to view (.)"
    onclick={() => graphState.centerNode()}
    aria-label="Fit nodes to view"
  >
    ⊡
  </button>
  <span>{Math.round(graphState.cameraPosition[2] * 10)}%</span>
</div>

<style>
  .zoom-indicator {
    position: absolute;
    bottom: 10px;
    display: flex;
    align-items: center;
    gap: 4px;
    font-family: var(--font-family);
    font-size: 0.75em;
    color: var(--color-text);
    opacity: 0.35;
    z-index: 10;
    transition: opacity 0.15s, right 0.2s;
    pointer-events: auto;
  }

  .zoom-indicator:hover {
    opacity: 0.9;
  }

  .fit-btn {
    background: none;
    border: none;
    color: inherit;
    cursor: pointer;
    font-size: 1.1em;
    line-height: 1;
    padding: 0;
  }
</style>
