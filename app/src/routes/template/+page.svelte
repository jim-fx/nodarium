<script lang="ts">
  import { humanizeNumber } from '#lib/helpers/index.ts';
  import { groupNode } from '#lib/node-registry/groupNode.ts';
  import { IndexDBCache, RemoteNodeRegistry } from '#lib/node-registry/index.ts';
  import Viewer from '#lib/result-viewer/Viewer.svelte';
  import { goto } from '$app/navigation';
  import { resolve } from '$app/paths';
  import { page } from '$app/state';
  import { debugNode, MemoryRuntimeExecutor } from '@nodarium/runtime';
  import { templates } from '@nodarium/runtime/templates';
  import { Spinner, toast } from '@nodarium/ui';
  import { createPerformanceStore } from '@nodarium/utils';
  import type { Group } from 'three';

  const nodeRegistry = new RemoteNodeRegistry('', new IndexDBCache('node-registry'), [
    debugNode,
    groupNode
  ]);
  const runtime = new MemoryRuntimeExecutor(nodeRegistry);
  const perf = createPerformanceStore();

  const vertices = $derived($perf?.at(-1)?.['total-vertices']?.[0] || 0);
  const faces = $derived($perf?.at(-1)?.['total-faces']?.[0] || 0);
  const requested = $derived(page.url.searchParams.get('template'));
  const selected = $derived(templates.find((t) => t.id === requested)?.id ?? 'fern');

  let scene = $state<Group>(null!);
  let viewer = $state<ReturnType<typeof Viewer>>();
  let isExecuting = $state(false);

  async function generate(reroll: boolean, template = selected) {
    const entry = templates.find((t) => t.id === template);
    if (!entry || !viewer) return;
    isExecuting = true;
    perf.startRun();
    try {
      const graph = structuredClone(await entry.load());
      await nodeRegistry.load(graph.nodes.map((n) => n.type));
      const settings = reroll
        ? { ...graph.settings, seed: Math.floor(Math.random() * 100000000) }
        : (graph.settings ?? {});
      viewer.update(await runtime.execute(graph, settings));
    } catch (error) {
      viewer.update(new Int32Array());
      toast(error instanceof Error ? error.message : String(error), 'error', 6000);
    } finally {
      perf.stopRun();
      isExecuting = false;
    }
  }

  function select(id: string) {
    if (id === selected) return;

    const url = resolve(`/template?template=${encodeURIComponent(id)}`);

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
      {#each templates as template (template.id)}
        <li>
          <button class:active={template.id === selected} onclick={() => select(template.id)}>
            {template.name}
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
