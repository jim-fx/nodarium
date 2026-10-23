<script lang="ts">
  import * as templates from '#lib/graph-templates/index.ts';
  import { humanizeNumber } from '#lib/helpers/index.ts';
  import { debugNode } from '#lib/node-registry/debugNode.ts';
  import { groupNode } from '#lib/node-registry/groupNode.ts';
  import { IndexDBCache, RemoteNodeRegistry } from '#lib/node-registry/index.ts';
  import Viewer from '#lib/result-viewer/Viewer.svelte';
  import { MemoryRuntimeExecutor } from '#lib/runtime/index.ts';
  import { goto } from '$app/navigation';
  import { resolve } from '$app/paths';
  import { page } from '$app/state';
  import type { Graph } from '@nodarium/types';
  import { Spinner, toast } from '@nodarium/ui';
  import { createPerformanceStore } from '@nodarium/utils';
  import type { Group } from 'three';

  // the stress tests are not plants
  const hidden = ['lottaFaces', 'lottaNodes', 'lottaNodesAndFaces', 'tutorial'];
  const entries = Object.entries(templates)
    .filter(([name, value]) => typeof value === 'object' && !hidden.includes(name))
    .map(([name, graph]) => ({ name, graph: graph as unknown as Graph }));

  const nodeRegistry = new RemoteNodeRegistry('', new IndexDBCache('node-registry'), [
    debugNode,
    groupNode
  ]);
  const runtime = new MemoryRuntimeExecutor(nodeRegistry);
  const perf = createPerformanceStore();

  const vertices = $derived($perf?.at(-1)?.['total-vertices']?.[0] || 0);
  const faces = $derived($perf?.at(-1)?.['total-faces']?.[0] || 0);
  const defaultTemplate = entries.find((e) => e.name === 'fern')?.name ?? entries[0].name;
  const requested = $derived(page.url.searchParams.get('template'));
  const selected = $derived(
    entries.find((entry) => entry.name === requested)?.name ?? defaultTemplate
  );

  let scene = $state<Group>(null!);
  let viewer = $state<ReturnType<typeof Viewer>>();
  let isExecuting = $state(false);

  function label(name: string) {
    return name.replace(/([A-Z])/g, ' $1').replace(/^./, (c) => c.toUpperCase());
  }

  async function generate(randomSeed: boolean, template = selected) {
    const entry = entries.find((e) => e.name === template);
    if (!entry || !viewer) return;
    isExecuting = true;
    perf.startRun();
    try {
      const graph = structuredClone($state.snapshot(entry.graph)) as Graph;
      await nodeRegistry.load(graph.nodes.map((n) => n.type));
      const settings = { ...(graph.settings ?? {}), randomSeed };
      viewer.update(await runtime.execute(graph, settings));
    } catch (error) {
      viewer.update(new Int32Array());
      toast(error instanceof Error ? error.message : String(error), 'error', 6000);
    } finally {
      perf.stopRun();
      isExecuting = false;
    }
  }

  function select(name: string) {
    if (name === selected) return;

    const url = resolve(`/template?template=${encodeURIComponent(name)}`);

    void goto(url, { replace: true, reset: false });
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key !== 'r' || event.ctrlKey || event.metaKey || event.altKey) return;
    generate(true);
  }

  $effect(() => {
    if (viewer) generate(false, selected);
  });
</script>

<svelte:document {onkeydown} />

<div class="page">
  <nav>
    <h1>Templates</h1>
    <ul>
      {#each entries as entry (entry.name)}
        <li>
          <button class:active={entry.name === selected} onclick={() => select(entry.name)}>
            {label(entry.name)}
          </button>
        </li>
      {/each}
    </ul>
    <p class="hint">Press <kbd>R</kbd> to regenerate</p>
  </nav>
  <main>
    <Viewer bind:scene bind:this={viewer} {perf} centerCamera />
    <div class="stats">
      <span>{humanizeNumber(vertices)} vertices</span>
      <span>{humanizeNumber(faces)} faces</span>
    </div>
    {#if isExecuting}
      <div class="spinner"><Spinner size={28} /></div>
    {/if}
  </main>
</div>

<style>
  .page {
    display: grid;
    grid-template-columns: 220px 1fr;
    height: 100vh;
    background: var(--layer-0, #111);
    color: var(--text-color, #eee);
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px 12px;
    overflow-y: auto;
    border-right: 1px solid var(--outline, #333);
    background: var(--layer-1, #181818);
  }
  h1 {
    margin: 0;
    padding: 0 8px;
    font-size: 14px;
    font-weight: 600;
    opacity: 0.7;
  }
  ul {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  button {
    width: 100%;
    padding: 8px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  button:hover {
    background: var(--layer-2, #262626);
  }
  button.active {
    background: var(--layer-3, #333);
    font-weight: 600;
  }
  .hint {
    margin: auto 0 0;
    padding: 0 8px;
    font-size: 12px;
    opacity: 0.6;
  }
  kbd {
    padding: 1px 5px;
    border: 1px solid currentColor;
    border-radius: 4px;
  }
  main {
    position: relative;
    min-width: 0;
  }
  .stats {
    position: absolute;
    right: 16px;
    bottom: 16px;
    z-index: 2;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    font-size: 12px;
    opacity: 0.7;
    pointer-events: none;
  }
  .spinner {
    position: absolute;
    top: 16px;
    right: 16px;
  }
</style>
