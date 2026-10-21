<script lang="ts">
  import { useTask } from '@threlte/core';
  import { getGraphState } from '../graph-state.svelte';

  const { el }: { el?: HTMLElement } = $props();

  const graphState = getGraphState();

  // Scale the HTML overlay in the render loop, so it uses the same camera
  // state as the HTML position and the canvas, instead of running a frame ahead.
  useTask(() => {
    if (!el) return;
    el.style.transform = `scale(${graphState.cameraPosition[2] * 0.1})`;
  }, { autoInvalidate: false });
</script>
