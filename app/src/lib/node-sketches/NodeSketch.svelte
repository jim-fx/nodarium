<script lang="ts">
  import type { NodeDefinition } from '@nodarium/types';
  import { MediaQuery } from 'svelte/reactivity';
  import {
    sketchFor,
    sketchPhase,
    type SketchPoint,
    type SketchShape,
    type SketchTone
  } from './sketches';

  const { node }: { node: NodeDefinition } = $props();

  const W = 92;
  const H = 68;

  const sketch = $derived(sketchFor(node.id));
  const reducedMotion = new MediaQuery('prefers-reduced-motion: reduce');

  let ms = $state(0);

  $effect(() => {
    if (!sketch || reducedMotion.current) return;
    const start = performance.now();
    let frame = requestAnimationFrame(function tick(now) {
      ms = now - start;
      frame = requestAnimationFrame(tick);
    });
    return () => cancelAnimationFrame(frame);
  });

  // without motion the sketch rests where the node has done its work
  const phase = $derived(reducedMotion.current ? { p: 1, cycle: 0 } : sketchPhase(ms));
  const frame = $derived(sketch?.draw(phase.p, phase.cycle));

  const STROKE: Record<SketchTone, string> = {
    base: 'stroke-current opacity-70',
    accent: 'stroke-selected',
    faint: 'stroke-current opacity-25'
  };
  const FILL: Record<SketchTone, string> = {
    base: 'fill-current opacity-70',
    accent: 'fill-selected',
    faint: 'fill-current opacity-25'
  };

  const toPx = ([x, y]: SketchPoint) => `${(x * W).toFixed(1)},${((1 - y) * H).toFixed(1)}`;
</script>

{#snippet panel(shapes: SketchShape[], label: string)}
  <figure class="m-0 flex flex-col items-center gap-1">
    <svg
      width={W}
      height={H}
      viewBox="0 0 {W} {H}"
      class="block rounded-[3px] border border-outline bg-layer-0"
      aria-hidden="true"
    >
      {#each shapes as shape, i (i)}
        {#if shape.kind === 'line'}
          <polyline
            points={shape.points.map(toPx).join(' ')}
            class="fill-none {STROKE[shape.tone]}"
            stroke-width={shape.width ?? 1}
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        {:else}
          {@const [cx, cy] = toPx(shape.at).split(',')}
          <circle {cx} {cy} r={shape.r ?? 2} class={FILL[shape.tone]} />
        {/if}
      {/each}
    </svg>
    <figcaption class="text-xs opacity-50">{label}</figcaption>
  </figure>
{/snippet}

{#if sketch && frame}
  <div class="mb-4 flex items-start gap-2" data-node-sketch={node.id}>
    {@render panel(frame.input, sketch.input)}
    <span class="i-[tabler--arrow-right] mt-7 block h-4 w-4 shrink-0 opacity-40"></span>
    {@render panel(frame.output, sketch.output)}
  </div>
{/if}
