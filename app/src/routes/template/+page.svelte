<script lang="ts">
  import GraphInterface, { type GraphOptions } from '#lib/graph-interface/index.ts';
  import { groupNode } from '#lib/node-registry/groupNode.ts';
  import { createAppRegistry } from '#lib/node-registry/index.ts';
  import { materials } from '#lib/result-viewer/materials/materials.ts';
  import ResultStats from '#lib/result-viewer/ResultStats.svelte';
  import Viewer from '#lib/result-viewer/Viewer.svelte';
  import { appSettings } from '#lib/settings/app-settings.svelte.ts';
  import {
    type ChangedValues,
    changedValues,
    editorGraph,
    withValues
  } from '#lib/template-values.ts';
  import { colors, systemColors } from '#lib/theme/colors.svelte.ts';
  import { goto } from '$app/navigation';
  import { resolve } from '$app/paths';
  import { page } from '$app/state';
  import { debugNode, graphLink, MemoryRuntimeExecutor } from '@nodarium/runtime';
  import { templates } from '@nodarium/runtime/templates';
  import type { Graph } from '@nodarium/types';
  import { Button, InputNumber, InputSelect, Spinner, toast } from '@nodarium/ui';
  import { createPerformanceStore } from '@nodarium/utils';
  import type { Group } from 'three';

  const registry = createAppRegistry([debugNode, groupNode]);
  const runtime = new MemoryRuntimeExecutor(registry);
  const perf = createPerformanceStore();

  const vertices = $derived($perf?.at(-1)?.['total-vertices']?.[0] || 0);
  const faces = $derived($perf?.at(-1)?.['total-faces']?.[0] || 0);
  const requested = $derived(page.url.searchParams.get('template'));
  const selected = $derived(templates.find((t) => t.id === requested)?.id ?? 'fern');

  let scene = $state<Group>(null!);
  let viewer = $state<ReturnType<typeof Viewer>>();
  let graphInterface = $state<ReturnType<typeof GraphInterface>>();
  let template = $state.raw<Graph>();
  /** What the user changed in the nodes, it stays when the seed or the time change */
  let values = $state.raw<ChangedValues>({});
  let time = $state(0.5);
  let seed = $state<number>();
  let isLoading = $state(false);
  let isExecuting = $state(false);
  let queued = false;

  const graph = $derived(template && withValues(template, values));
  const hasTime = $derived(template?.nodes.some((n) => n.type === 'max/core/time') ?? false);
  const hasChanges = $derived(Object.keys(values).length > 0);

  const MATERIAL_KEY = 'template.material';
  function readMaterial() {
    try {
      const stored = localStorage.getItem(MATERIAL_KEY);
      if (materials.some((m) => m.id === stored)) return stored!;
    } catch {
      // without storage the page starts with the material of the editor
    }
    return appSettings.value.material.id;
  }
  let material = $state(readMaterial());
  const materialIndex = $derived(Math.max(0, materials.findIndex((m) => m.id === material)));
  function selectMaterial(index: number) {
    material = materials[index]?.id ?? material;
    try {
      localStorage.setItem(MATERIAL_KEY, material);
    } catch {
      // the material is still used for this visit
    }
  }

  const graphOptions: GraphOptions = {
    get backgroundType() {
      return appSettings.value.nodeInterface.backgroundType;
    },
    snapToGrid: false,
    showHelp: false,
    showDebugInfo: false,
    get theme() {
      return appSettings.value.theme;
    },
    colors,
    systemColors,
    nodeTimings: {},
    valuesOnly: true
  };

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
      template = loaded;
      values = {};
      time = Number(loaded.settings?.['time.t'] ?? 0.5);
      seed = undefined;
    } catch (error) {
      if (id !== selected) return;
      template = undefined;
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

  function edited(result: Graph) {
    if (!template) return;
    const next = changedValues(template, result);
    if (JSON.stringify(next) === JSON.stringify(values)) return;
    values = next;
    void run();
  }

  function reset() {
    if (!template) return;
    values = {};
    void graphInterface?.manager.load(structuredClone(template));
    void run();
  }

  let editorLink = $state<string>();
  $effect(() => {
    if (!graph) return;
    const entry = templates.find((t) => t.id === selected);
    const opened = editorGraph(template!, values, {
      title: entry?.name ?? selected,
      seed,
      time: hasTime ? time : undefined
    });
    let current = true;
    graphLink(opened, location.origin + resolve('/')).then((link) => {
      if (current) editorLink = link;
    });
    return () => (current = false);
  });

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
    if ((event.target as Element | null)?.closest?.('input, textarea, select')) return;
    seed = Math.floor(Math.random() * 100000000);
    void run();
  }

  $effect(() => {
    if (viewer) void load(selected);
  });
</script>

<svelte:document {onkeydown} />

<div class="grid h-dvh bg-layer-0 text-text grid-rows-[auto_minmax(0,1fr)] md:grid-cols-[220px_minmax(0,1fr)] md:grid-rows-1">
  <nav class="flex gap-3 overflow-auto border-b border-outline bg-layer-1 p-2 md:flex-col md:border-r md:border-b-0 md:px-3 md:py-4">
    <h1 class="m-0 hidden px-2 text-sm font-semibold opacity-70 md:block">Templates</h1>
    <ul class="m-0 flex list-none gap-0.5 p-0 md:flex-col">
      {#each templates as entry (entry.id)}
        <li>
          <button
            class="w-full cursor-pointer rounded-md border-0 p-2 text-left whitespace-nowrap text-inherit hover:bg-layer-2 {entry.id === selected ? 'bg-layer-2 font-semibold' : 'bg-transparent'}"
            aria-current={entry.id === selected ? 'page' : undefined}
            onclick={() => select(entry.id)}
          >
            {entry.name}
          </button>
        </li>
      {/each}
    </ul>
    <p class="mt-auto mb-0 hidden px-2 text-xs opacity-60 md:block">
      Press <kbd class="rounded border border-current px-1">R</kbd> to regenerate
    </p>
  </nav>
  <main class="grid min-w-0 grid-rows-2 md:grid-cols-2 md:grid-rows-1">
    <div class="relative min-h-0">
      <Viewer bind:scene bind:this={viewer} {perf} {material} centerCamera />
      <div class="absolute top-3 left-3 z-2 flex items-center gap-2 text-xs">
        <label for="material" class="opacity-70">Material</label>
        <InputSelect
          id="material"
          options={materials.map((m) => m.name)}
          bind:value={() => materialIndex, selectMaterial}
        />
      </div>
      <ResultStats {vertices} {faces} class="absolute right-4 bottom-4 items-end" />
      {#if hasTime}
        <div class="absolute bottom-4 left-4 z-2 flex w-[200px] items-center gap-2 text-xs md:left-1/2 md:-translate-x-1/2">
          <label for="time" class="opacity-70">Time</label>
          <div class="flex-1">
            <InputNumber id="time" bind:value={() => time, scrub} min={0} max={1} step={0.01} />
          </div>
        </div>
      {/if}
      {#if isLoading || isExecuting}
        <div class="absolute top-4 right-4"><Spinner size={28} /></div>
      {/if}
    </div>
    <div class="relative min-h-0 border-t border-outline md:border-t-0 md:border-l">
      {#if template}
        {#key template}
          <GraphInterface
            graph={structuredClone(template)}
            bind:this={graphInterface}
            {registry}
            options={graphOptions}
            onresult={edited}
          />
        {/key}
      {/if}
      <div class="absolute top-3 right-3 z-10 flex gap-2">
        <Button size="sm" disabled={!hasChanges} onclick={reset}>Reset</Button>
        <Button
          size="sm"
          variant="primary"
          disabled={!editorLink}
          onclick={() => window.open(editorLink, '_blank')}
        >
          Open in editor
        </Button>
      </div>
    </div>
  </main>
</div>
