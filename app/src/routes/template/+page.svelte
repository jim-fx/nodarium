<script lang="ts">
  import { groupNode } from '#lib/node-registry/groupNode.ts';
  import { createAppRegistry } from '#lib/node-registry/index.ts';
  import ResultStats from '#lib/result-viewer/ResultStats.svelte';
  import Viewer from '#lib/result-viewer/Viewer.svelte';
  import { goto } from '$app/navigation';
  import { resolve } from '$app/paths';
  import { page } from '$app/state';
  import { debugNode, MemoryRuntimeExecutor } from '@nodarium/runtime';
  import { templates } from '@nodarium/runtime/templates';
  import type { Graph } from '@nodarium/types';
  import { InputNumber, Spinner, toast } from '@nodarium/ui';
  import { createPerformanceStore } from '@nodarium/utils';
  import type { Group } from 'three';

  const runtime = new MemoryRuntimeExecutor(createAppRegistry([debugNode, groupNode]));
  const perf = createPerformanceStore();

  const vertices = $derived($perf?.at(-1)?.['total-vertices']?.[0] || 0);
  const faces = $derived($perf?.at(-1)?.['total-faces']?.[0] || 0);
  const requested = $derived(page.url.searchParams.get('template'));
  const selected = $derived(templates.find((t) => t.id === requested)?.id ?? 'fern');

  let scene = $state<Group>(null!);
  let viewer = $state<ReturnType<typeof Viewer>>();
  let graph = $state.raw<Graph>();
  let time = $state(0.5);
  let seed = $state<number>();
  let isLoading = $state(false);
  let isExecuting = $state(false);
  let queued = false;

  const hasTime = $derived(graph?.nodes.some((n) => n.type === 'max/myzel/time') ?? false);

  function fail(error: unknown) {
    viewer?.update(new Int32Array());
    toast(error instanceof Error ? error.message : String(error), 'error', 6000);
  }

  async function load(id: string) {
    const entry = templates.find((t) => t.id === id);
    if (!entry) return;
    isLoading = true;
    try {
      const loaded = structuredClone(await entry.load());
      if (id !== selected) return;
      graph = loaded;
      time = Number(loaded.settings?.['time.t'] ?? 0.5);
      seed = undefined;
    } catch (error) {
      if (id !== selected) return;
      graph = undefined;
      fail(error);
    } finally {
      if (id === selected) isLoading = false;
    }
    await run();
  }

  async function run() {
    if (!graph || !viewer) return;
    // a scrub asks for more builds than there is time for, only the last one asked for is built
    if (isExecuting) {
      queued = true;
      return;
    }
    isExecuting = true;
    do {
      queued = false;
      const settings = {
        ...graph.settings,
        ...(hasTime && { 'time.t': time }),
        ...(seed !== undefined && { seed })
      };
      perf.startRun();
      try {
        viewer.update(await runtime.execute(graph, settings));
      } catch (error) {
        fail(error);
      } finally {
        perf.stopRun();
      }
    } while (queued && graph);
    isExecuting = false;
  }

  function scrub(t: number) {
    time = t;
    void run();
  }

  function select(id: string) {
    if (id === selected) return;

    const url = resolve(`/template?template=${encodeURIComponent(id)}`);

    void goto(url, { replace: true, reset: false });
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key !== 'r' || event.ctrlKey || event.metaKey || event.altKey) return;
    seed = Math.floor(Math.random() * 100000000);
    void run();
  }

  $effect(() => {
    if (viewer) void load(selected);
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
    <ResultStats {vertices} {faces} class="absolute right-4 bottom-4 items-end" />
    {#if hasTime}
      <div class="time">
        <label for="time">Time</label>
        <InputNumber id="time" bind:value={() => time, scrub} min={0} max={1} step={0.01} />
      </div>
    {/if}
    {#if isLoading || isExecuting}
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
  .time {
    position: absolute;
    bottom: 16px;
    left: 50%;
    z-index: 2;
    display: flex;
    align-items: center;
    gap: 8px;
    width: 220px;
    font-size: 12px;
    transform: translateX(-50%);
  }
  .time label {
    opacity: 0.7;
  }
  .time :global(.component-wrapper) {
    flex: 1;
  }
  .spinner {
    position: absolute;
    top: 16px;
    right: 16px;
  }
</style>
