<script lang="ts">
  import GraphInterface from '$lib/graph-interface';
  import * as templates from '$lib/graph-templates';
  import Grid from '$lib/grid';
  import { debounceAsyncFunction } from '$lib/helpers';
  import { createKeyMap } from '$lib/helpers/createKeyMap';
  import { debugNode } from '$lib/node-registry/debugNode';
  import { groupNode } from '$lib/node-registry/groupNode.js';
  import { IndexDBCache, RemoteNodeRegistry } from '$lib/node-registry/index';

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
  import GroupSettings from '$lib/sidebar/panels/GroupSettings.svelte';
  import Keymap from '$lib/sidebar/panels/Keymap.svelte';
  import { panelState } from '$lib/sidebar/PanelState.svelte';
  import Sidebar from '$lib/sidebar/Sidebar.svelte';
  import { tutorialConfig } from '$lib/tutorial/tutorial-config';
  import { Planty } from '@nodarium/planty';
  import type { Graph, NodeInstance } from '@nodarium/types';
  import { Spinner, Toast, toast } from '@nodarium/ui';
  import { createPerformanceStore } from '@nodarium/utils';
  import type { Group } from 'three';

  let performanceStore = createPerformanceStore();
  let planty = $state<ReturnType<typeof Planty>>();
  let pendingSave = false;

  const { data } = $props();

  const registryCache = new IndexDBCache('node-registry');

  const nodeRegistry = new RemoteNodeRegistry('', registryCache, [debugNode, groupNode]);
  const workerRuntime = new WorkerRuntimeExecutor();
  const runtimeCache = new MemoryRuntimeCache();
  const memoryRuntime = new MemoryRuntimeExecutor(nodeRegistry, runtimeCache);
  memoryRuntime.perf = performanceStore;
  const pm = new ProjectManager();

  const runtime = $derived(
    appSettings.value.debug.useWorker ? workerRuntime : memoryRuntime
  );

  $effect(() => {
    workerRuntime.useRegistryCache = appSettings.value.debug.cache.useRegistryCache;
    workerRuntime.useRuntimeCache = appSettings.value.debug.cache.useRuntimeCache;

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

  $effect(() => {
    const handler = (e: BeforeUnloadEvent) => {
      if (pendingSave) {
        e.preventDefault();
      }
    };
    window.addEventListener('beforeunload', handler);
    return () => window.removeEventListener('beforeunload', handler);
  });

  let activeNode = $state<NodeInstance | undefined>(undefined);
  let scene = $state<Group>(null!);
  let isExecuting = $state(false);

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
  $effect(() => {
    if (graphSettings && graphSettingTypes && manager?.loaded) {
      manager?.setSettings($state.snapshot(graphSettings));
    }
  });

  let timeout: ReturnType<typeof setTimeout>;

  async function update(
    g: Graph,
    s: Record<string, unknown> = $state.snapshot(graphSettings)
  ) {
    if (timeout) clearTimeout(timeout);
    timeout = setTimeout(() => {
      isExecuting = true;
    }, 100);
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
      const msg = error instanceof Error ? error.message : String(error);
      toast(`Execution failed: ${msg}`, 'error');
    } finally {
      clearTimeout(timeout);
      isExecuting = false;
      performanceStore.stopRun();
    }
  }

  const handleUpdate = debounceAsyncFunction(update);

  function handleSettingsButton(id: string) {
    switch (id) {
      case 'general.clippy':
        planty?.start();
        break;
      case 'general.debug.stressTest.loadGrid':
        manager.load(
          templates.grid(
            appSettings.value.debug.stressTest.amount,
            appSettings.value.debug.stressTest.amount
          )
        );
        break;
      case 'general.debug.stressTest.loadTree':
        manager.load(templates.tree(appSettings.value.debug.stressTest.amount));
        break;
      case 'general.debug.stressTest.lottaFaces':
        manager.load(templates.lottaFaces as unknown as Graph);
        break;
      case 'general.debug.stressTest.lottaNodes':
        manager.load(templates.lottaNodes as unknown as Graph);
        break;
      case 'general.debug.stressTest.lottaNodesAndFaces':
        manager.load(templates.lottaNodesAndFaces as unknown as Graph);
        break;
      default:
    }
  }
</script>

<svelte:document onkeydown={applicationKeymap.handleKeyboardEvent} />

<Planty
  bind:this={planty}
  config={tutorialConfig}
  actions={{
    'setup-default': () => {
      console.log('setup-default');
      const ts = new Date().toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' });
      pm.handleCreateProject(
        structuredClone(templates.defaultPlant) as unknown as Graph,
        `Tutorial Project (${ts})`
      );
    },
    'load-tutorial-template': () => {
      console.log('load-tutorial-template');
      if (!pm.graph) return;
      const g = structuredClone(templates.tutorial) as unknown as Graph;
      g.id = pm.graph.id;
      g.meta = { ...pm.graph.meta };
      manager.load(g);
      graphInterface.state.centerNode(graphInterface.manager.getAllNodes()[0]);
    },
    'open-github-nodes': () => {
      console.log('open-github-nodes');
      window.open(
        'https://github.com/jim-fx/nodarium/tree/main/nodes/max/plantarium',
        '__blank'
      );
    }
  }}
  hooks={{
    'action:add_stem_node': (cb) => {
      const unsub = manager.on('save', () => {
        const allNodes = graphInterface.manager.getAllNodes();
        const stemNode = allNodes.find(n => n.type === 'max/plantarium/stem');
        if (stemNode && graphInterface.manager.edges.length) {
          unsub();
          (cb as () => void)();
        }
      });
    },
    'action:add_noise_node': (cb) => {
      const unsub = manager.on('save', () => {
        const allNodes = graphInterface.manager.getAllNodes();
        const noiseNode = allNodes.find(n => n.type === 'max/plantarium/noise');
        if (noiseNode && graphInterface.manager.edges.length > 1) {
          unsub();
          (cb as () => void)();
        }
      });
    },
    'action:add_random_node': (cb) => {
      const unsub = manager.on('save', () => {
        const allNodes = graphInterface.manager.getAllNodes();
        const noiseNode = allNodes.find(n => n.type === 'max/plantarium/random');
        if (noiseNode && graphInterface.manager.edges.length > 2) {
          unsub();
          (cb as () => void)();
        }
      });
    },
    'action:prompt_regenerate': (cb) => {
      function handleKeydown(e: KeyboardEvent) {
        if (e.key === 'r') {
          window.removeEventListener('keydown', handleKeydown);
          (cb as () => void)();
        }
      }
      window.addEventListener('keydown', handleKeydown);
    },
    'before:save_project': () => panelState.setActivePanel('projects'),
    'before:export_tour': () => panelState.setActivePanel('exports'),
    'before:shortcuts_tour': () => panelState.setActivePanel('shortcuts'),
    'after:save_project': () => panelState.setActivePanel('graph-settings'),
    'before:tour_runtime_nerd': () => panelState.setActivePanel('general')
  }}
/>

<div class="wrapper manager-{manager?.status}">
  <header></header>
  <Grid.Row>
    <Grid.Cell>
      <div class="viewer-cell">
        <Viewer
          bind:scene
          bind:this={viewerComponent}
          perf={performanceStore}
          debugData={debugData}
          centerCamera={appSettings.value.centerCamera}
        />
        {#if isExecuting}
          <div class="viewer-spinner" aria-label="Executing graph">
            <Spinner size={28} />
          </div>
        {/if}
      </div>
    </Grid.Cell>
    <Grid.Cell>
      {#if pm.graph}
        {#key pm.graph.id}
          <GraphInterface
            graph={pm.graph}
            bind:this={graphInterface}
            registry={nodeRegistry}
            safePadding={{ right: sidebarOpen ? 321 : undefined }}
            backgroundType={appSettings.value.nodeInterface.backgroundType}
            snapToGrid={appSettings.value.nodeInterface.snapToGrid}
            bind:activeNode
            bind:showHelp={appSettings.value.nodeInterface.showHelp}
            bind:settings={graphSettings}
            bind:settingTypes={graphSettingTypes}
            onsave={async (g) => { pendingSave = true; await pm.saveGraph(g); pendingSave = false; }}
            onresult={(result) => handleUpdate(result as Graph)}
          />
        {/key}
      {/if}
      <Sidebar bind:open={sidebarOpen}>
        <Panel id="general" title="General" icon="i-[tabler--settings]">
          <NestedSettings
            id="general"
            onButtonClick={handleSettingsButton}
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
          {#if manager?.status === 'idle'}
            <GraphSource graph={manager.serialize()} />
          {/if}
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
          <span class="block h-[1px]"></span>
          <NestedSettings
            id="graph-settings"
            type={graphSettingTypes}
            bind:value={graphSettings}
          />
          {#key activeNode}
            <ActiveNodeSettings {manager} bind:node={activeNode} />
            <GroupSettings graphState={graphInterface?.state} {manager} bind:node={activeNode} />
          {/key}
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

<Toast />

<style>
  header {
    background-color: var(--color-layer-1);
    display: flex;
    align-items: center;
    padding: 0 8px;
  }

  .tutorial-btn {
    background: none;
    border: none;
    cursor: pointer;
    font-size: 18px;
    padding: 4px 6px;
    border-radius: 6px;
    opacity: 0.7;
    transition: opacity 0.15s, background 0.15s;
  }

  .tutorial-btn:hover {
    opacity: 1;
    background: rgba(255, 255, 255, 0.08);
  }

  .wrapper {
    height: 100vh;
    width: 100vw;
    color: white;
    display: grid;
    grid-template-rows: 0px 1fr;
  }

  .viewer-cell {
    position: relative;
    height: 100%;
  }

  .viewer-spinner {
    position: absolute;
    bottom: 12px;
    right: 12px;
    color: var(--color-text, #cecece);
    opacity: 0.6;
    pointer-events: none;
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
