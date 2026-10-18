<script lang="ts">
  import { localState } from '$lib/helpers/localState.svelte';
  import * as db from '$lib/project-manager/project-database.svelte';
  import type { Graph } from '@nodarium/types';
  import { Canvas } from '@threlte/core';
  import { onMount } from 'svelte';
  import { SvelteMap } from 'svelte/reactivity';
  import { PCFShadowMap } from 'three';
  import { endpointSymbol } from 'vite-plugin-comlink/symbol';
  import { GARDEN_STYLES, type GardenStyle, MAX_PLANTS } from './config';
  import { createGardenWorker } from './garden-worker';
  import GardenScene from './GardenScene.svelte';

  const style = localState<GardenStyle>('garden.style', 'zen');
  // the stored value may come from an older version with other styles
  const activeStyle = $derived(GARDEN_STYLES.includes(style.value) ? style.value : 'zen');

  let projects = $state.raw<Graph[]>([]);
  /** encoded geometry per project, null if the project could not be executed */
  const results = new SvelteMap<number, Int32Array | null>();

  onMount(() => {
    const worker = createGardenWorker();
    let cancelled = false;

    (async () => {
      const graphs = await db.getGraphs();
      projects = graphs
        .sort((a, b) => (b.meta?.lastModified ?? '').localeCompare(a.meta?.lastModified ?? ''))
        .slice(0, MAX_PLANTS);

      // one after another, so the plants grow in one by one
      for (const project of projects) {
        if (cancelled) return;
        try {
          results.set(project.id, await worker.executeGraph(project));
        } catch (error) {
          console.warn(`[garden] could not execute project ${project.id}`, error);
          results.set(project.id, null);
        }
      }
    })();

    return () => {
      cancelled = true;
      worker[endpointSymbol].terminate();
    };
  });
</script>

<div class="relative h-screen w-screen bg-neutral-900">
  <Canvas shadows={PCFShadowMap} renderMode="always">
    <!-- every style builds its own scene, the plants grow in again -->
    {#key activeStyle}
      <GardenScene style={activeStyle} {projects} {results} />
    {/key}
  </Canvas>

  <div class="absolute top-4 right-4 flex overflow-hidden rounded-[5px] border border-neutral-400 bg-neutral-500 text-xs">
    {#each GARDEN_STYLES as option (option)}
      <button
        class="
          h-8 cursor-pointer border-none px-3 capitalize font-(--font-family) {activeStyle === option
          ? 'bg-neutral-400 text-neutral-100'
          : 'bg-transparent text-neutral-300 hover:text-neutral-200'}
        "
        onclick={() => (style.value = option)}
      >
        {option}
      </button>
    {/each}
  </div>
</div>
