<script lang="ts">
  import GraphInterface from '$lib/graph-interface';
  import * as templates from '$lib/graph-templates';
  import Grid from '$lib/grid';
  import { debounceAsyncFunction } from '$lib/helpers';
  import { createKeyMap } from '$lib/helpers/createKeyMap';
  import { debugNode } from '$lib/node-registry/debugNode.js';
  import { IndexDBCache, RemoteNodeRegistry } from '$lib/node-registry/index';
  import NodeStore from '$lib/node-store/NodeStore.svelte';
  import PerformanceViewer from '$lib/performance/PerformanceViewer.svelte';
  import { ProjectManager } from '$lib/project-manager/project-manager.svelte';
  import ProjectManagerEl from '$lib/project-manager/ProjectManager.svelte';
  import Viewer from '$lib/result-viewer/Viewer.svelte';
  import { MemoryRuntimeCache, MemoryRuntimeExecutor, WorkerRuntimeExecutor } from '$lib/runtime';
  import type { SettingsValue } from '$lib/settings';
  import { appSettings, AppSettingTypes } from '$lib/settings/app-settings.svelte';
  import NestedSettings from '$lib/settings/NestedSettings.svelte';
  import Panel from '$lib/sidebar/Panel.svelte';
  import ActiveNodeSettings from '$lib/sidebar/panels/ActiveNodeSettings.svelte';
  import BenchmarkPanel from '$lib/sidebar/panels/BenchmarkPanel.svelte';
  import Changelog from '$lib/sidebar/panels/Changelog.svelte';
  import ExportSettings from '$lib/sidebar/panels/ExportSettings.svelte';
  import GraphSource from '$lib/sidebar/panels/GraphSource.svelte';
  import Keymap from '$lib/sidebar/panels/Keymap.svelte';
  import Sidebar from '$lib/sidebar/Sidebar.svelte';
  import type { Graph, NodeInstance } from '@nodarium/types';
  import { createPerformanceStore } from '@nodarium/utils';
  import { onMount } from 'svelte';
  import type { Group } from 'three';

  let performanceStore = createPerformanceStore();

  const { data } = $props();

  const registryCache = new IndexDBCache('node-registry');

  const nodeRegistry = new RemoteNodeRegistry('', registryCache, [debugNode]);
  const workerRuntime = new WorkerRuntimeExecutor();
  const runtimeCache = new MemoryRuntimeCache();
  const memoryRuntime = new MemoryRuntimeExecutor(nodeRegistry, runtimeCache);
  memoryRuntime.perf = performanceStore;
  const pm = new ProjectManager();

  const runtime = $derived(
    appSettings.value.debug.useWorker ? workerRuntime : memoryRuntime
  );

  $effect(() => {
    workerRuntime.useRegistryCache = appSettings.value.debug.cache.useRuntimeCache;
    workerRuntime.useRuntimeCache = appSettings.value.debug.cache.useRegistryCache;

    if (appSettings.value.debug.cache.useRegistryCache) {
      nodeRegistry.cache = registryCache;
    } else {
      nodeRegistry.cache = undefined;
    }

    if (appSettings.value.debug.cache.useRuntimeCache) {
      memoryRuntime.cache = runtimeCache;
    } else {
      memoryRuntime.cache = undefined;
    }
  });

  let activeNode = $state<NodeInstance | undefined>(undefined);
  let scene = $state<Group>(null!);

  let sidebarOpen = $state(false);
  let graphInterface = $state<ReturnType<typeof GraphInterface>>(null!);
  let viewerComponent = $state<ReturnType<typeof Viewer>>();
  let debugData = $state<Record<number, { type: string; data: Int32Array }>>();
  const manager = $derived(graphInterface?.manager);

  async function randomGenerate() {
    if (!manager) return;
    const g = manager.serialize();
    const s = { ...graphSettings, randomSeed: true };
    await handleUpdate(g, s);
  }

  let applicationKeymap = createKeyMap([
    {
      key: 'r',
      description: 'Regenerate the plant model',
      callback: () => randomGenerate()
    }
  ]);

  let graphSettings = $state<SettingsValue>({});
  let graphSettingTypes = $state({
    randomSeed: { type: 'boolean', value: false }
  });

  async function update(
    g: Graph,
    s: Record<string, unknown> = $state.snapshot(graphSettings)
  ) {
    performanceStore.startRun();
    try {
      let a = performance.now();
      const graphResult = await runtime.execute(g, s);
      let b = performance.now();

      if (appSettings.value.debug.useWorker) {
        let perfData = await runtime.getPerformanceData();
        debugData = await runtime.getDebugData();
        let lastRun = perfData?.at(-1);
        if (lastRun?.total) {
          lastRun.runtime = lastRun.total;
          delete lastRun.total;
          performanceStore.mergeData(lastRun);
          performanceStore.addPoint(
            'worker-transfer',
            b - a - lastRun.runtime[0]
          );
        }
      }
      viewerComponent?.update(graphResult);
    } catch (error) {
      console.log('errors', error);
    } finally {
      performanceStore.stopRun();
    }
  }

  const handleUpdate = debounceAsyncFunction(update);

  onMount(() => {
    appSettings.value.debug.stressTest = {
      ...appSettings.value.debug.stressTest,
      loadGrid: () => {
        manager.load(
          templates.grid(
            appSettings.value.debug.stressTest.amount,
            appSettings.value.debug.stressTest.amount
          )
        );
      },
      loadTree: () => {
        manager.load(templates.tree(appSettings.value.debug.stressTest.amount));
      },
      lottaFaces: () => {
        manager.load(templates.lottaFaces as unknown as Graph);
      },
      lottaNodes: () => {
        manager.load(templates.lottaNodes as unknown as Graph);
      },
      lottaNodesAndFaces: () => {
        manager.load(templates.lottaNodesAndFaces as unknown as Graph);
      }
    };
  });
</script>

<svelte:document onkeydown={applicationKeymap.handleKeyboardEvent} />

<div class="wrapper manager-{manager?.status}">
  <header></header>
  <Grid.Row>
    <Grid.Cell>
      <Viewer
        bind:scene
        bind:this={viewerComponent}
        perf={performanceStore}
        debugData={debugData}
        centerCamera={appSettings.value.centerCamera}
      />
    </Grid.Cell>
    <Grid.Cell>
      {#if pm.graph}
        <GraphInterface
          graph={pm.graph}
          bind:this={graphInterface}
          registry={nodeRegistry}
          addMenuPadding={{ right: sidebarOpen ? 330 : undefined }}
          backgroundType={appSettings.value.nodeInterface.backgroundType}
          snapToGrid={appSettings.value.nodeInterface.snapToGrid}
          bind:activeNode
          bind:showHelp={appSettings.value.nodeInterface.showHelp}
          bind:settings={graphSettings}
          bind:settingTypes={graphSettingTypes}
          onsave={(g) => pm.saveGraph(g)}
          onresult={(result) => handleUpdate(result as Graph)}
        />
      {/if}
      <Sidebar bind:open={sidebarOpen}>
        <Panel id="general" title="General" icon="i-[tabler--settings]">
          <NestedSettings
            id="general"
            bind:value={appSettings.value}
            type={AppSettingTypes}
          />
        </Panel>
        <Panel
          id="shortcuts"
          title="Keyboard Shortcuts"
          icon="i-[tabler--keyboard]"
        >
          <Keymap
            keymaps={[
              { keymap: applicationKeymap, title: 'Application' },
              { keymap: graphInterface?.keymap, title: 'Node-Editor' }
            ]}
          />
        </Panel>
        <Panel id="exports" title="Exporter" icon="i-[tabler--package-export]">
          <ExportSettings {scene} />
        </Panel>
        <Panel
          id="node-store"
          title="Node Store"
          icon="i-[tabler--database] bg-green-400"
        >
          <NodeStore registry={nodeRegistry} />
        </Panel>
        <Panel
          id="performance"
          title="Performance"
          hidden={!appSettings.value.debug.advancedMode}
          icon="i-[tabler--brand-speedtest] bg-red-400"
        >
          {#if $performanceStore}
            <PerformanceViewer data={$performanceStore} />
          {/if}
        </Panel>
        <Panel id="projects" icon="i-[tabler--folder-open]">
          <ProjectManagerEl projectManager={pm} />
        </Panel>
        <Panel
          id="graph-source"
          title="Graph Source"
          hidden={!appSettings.value.debug.advancedMode}
          icon="i-[tabler--code]"
        >
          <GraphSource graph={pm.graph ?? manager?.serialize()} />
        </Panel>
        <Panel
          id="benchmark"
          title="Benchmark"
          hidden={!appSettings.value.debug.advancedMode}
          icon="i-[tabler--graph] bg-red-400"
        >
          <BenchmarkPanel run={randomGenerate} />
        </Panel>
        <Panel
          id="graph-settings"
          title="Graph Settings"
          icon="i-[custom--graph] bg-blue-400"
        >
          <NestedSettings
            id="graph-settings"
            type={graphSettingTypes}
            bind:value={graphSettings}
          />
          <ActiveNodeSettings {manager} bind:node={activeNode} />
        </Panel>
        <Panel
          id="changelog"
          title="Changelog"
          icon="i-[tabler--file-text-spark] bg-green-400"
        >
          <Changelog git={data.git} changelog={data.changelog} />
        </Panel>
      </Sidebar>
    </Grid.Cell>
  </Grid.Row>
</div>

<style>
  header {
    background-color: var(--color-layer-1);
  }

  .wrapper {
    height: 100vh;
    width: 100vw;
    color: white;
    display: grid;
    grid-template-rows: 0px 1fr;
  }

  .wrapper :global(canvas) {
    transition: opacity 0.3s ease;
    opacity: 1;
  }

  .manager-loading :global(.graph-wrapper),
  .manager-loading :global(canvas) {
    opacity: 0.2;
    pointer-events: none;
  }

  :global(html) {
    background: rgb(13, 19, 32);
    background: linear-gradient(
      180deg,
      rgba(13, 19, 32, 1) 0%,
      rgba(8, 12, 21, 1) 100%
    );
  }

  :global(body) {
    margin: 0;
    position: relative;
  }
</style>
