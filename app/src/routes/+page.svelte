<script lang="ts">
  import GraphInterface from '$lib/graph-interface';
  import * as templates from '$lib/graph-templates';
  import Grid from '$lib/grid';
  import { debounceAsyncFunction } from '$lib/helpers';
  import { createKeyMap } from '$lib/helpers/createKeyMap';
  import { localState } from '$lib/helpers/localState.svelte';
  import { debugNode } from '$lib/node-registry/debugNode';
  import { groupNode } from '$lib/node-registry/groupNode.js';
  import { IndexDBCache, RemoteNodeRegistry } from '$lib/node-registry/index';

  import PerformanceViewer from '$lib/performance/PerformanceViewer.svelte';
  import { ProjectManager } from '$lib/project-manager/project-manager.svelte';
  import ProjectBrowser from '$lib/project-manager/ProjectBrowser.svelte';
  import Viewer from '$lib/result-viewer/Viewer.svelte';
  import {
    getErrorInfo,
    MemoryRuntimeCache,
    MemoryRuntimeExecutor,
    WorkerRuntimeExecutor
  } from '$lib/runtime';
  import { errorLog } from '$lib/runtime/error-log.svelte';
  import type { SettingsValue } from '$lib/settings';
  import { appSettings, AppSettingTypes } from '$lib/settings/app-settings.svelte';
  import MaterialPicker from '$lib/settings/MaterialPicker.svelte';
  import NestedSettings from '$lib/settings/NestedSettings.svelte';
  import Panel from '$lib/sidebar/Panel.svelte';
  import ActiveNodeSettings from '$lib/sidebar/panels/ActiveNodeSettings.svelte';
  import BenchmarkPanel from '$lib/sidebar/panels/BenchmarkPanel.svelte';
  import Changelog from '$lib/sidebar/panels/Changelog.svelte';
  import ErrorLog from '$lib/sidebar/panels/ErrorLog.svelte';
  import ExportSettings from '$lib/sidebar/panels/ExportSettings.svelte';
  import GraphSource from '$lib/sidebar/panels/GraphSource.svelte';
  import GroupSettings from '$lib/sidebar/panels/GroupSettings.svelte';
  import Keymap from '$lib/sidebar/panels/Keymap.svelte';
  import { panelState } from '$lib/sidebar/PanelState.svelte';
  import Sidebar from '$lib/sidebar/Sidebar.svelte';
  import TitleBar from '$lib/title-bar/TitleBar.svelte';
  import { tutorialChecks } from '$lib/tutorial/tutorial-checks';
  import { tutorialConfig } from '$lib/tutorial/tutorial-config';
  import { Planty, type StepActionHook } from '@nodarium/planty';
  import type { Edge, Graph, NodeInstance } from '@nodarium/types';
  import { Spinner, Toast, toast } from '@nodarium/ui';
  import { createPerformanceStore } from '@nodarium/utils';
  import type { Group } from 'three';

  let performanceStore = createPerformanceStore();
  let planty = $state<ReturnType<typeof Planty>>();
  let pendingSave = $state(false);
  let titleBar = $state<ReturnType<typeof TitleBar>>();
  let projectBrowserOpen = $state(false);
  /** The project the tutorial builds in, reused on every run (-1 = none yet) */
  const tutorialProjectId = localState('node.tutorialProjectId', -1);
  /** Set while the tutorial waits for the user to press R */
  let onRegenerate: (() => void) | undefined;

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
  let nodeTimings = $state<Record<number, number>>({});
  let errorNodeId = $state<number>();
  const manager = $derived(graphInterface?.manager);

  pm.onrenameactive = (title) => {
    if (manager) manager.meta = { ...manager.meta, title };
  };

  /** Tutorial step hook that advances once the graph passes `check` */
  function waitForGraph(check: (edges: Edge[]) => boolean): StepActionHook {
    return (advance) => {
      const m = manager;
      if (!m) return;
      if (check(m.edges)) {
        advance();
        return;
      }
      return m.on('save', () => {
        if (check(m.edges)) advance();
      });
    };
  }

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
      callback: () => {
        randomGenerate();
        onRegenerate?.();
      }
    },
    {
      key: 'o',
      ctrl: true,
      preventDefault: true,
      description: 'Open project',
      callback: () => (projectBrowserOpen = true)
    },
    {
      key: 'n',
      alt: true,
      preventDefault: true,
      description: 'New project',
      callback: () => (projectBrowserOpen = true)
    },
    {
      key: 'F2',
      preventDefault: true,
      description: 'Rename project',
      callback: () => titleBar?.startRename()
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
      nodeTimings = await runtime.getNodeTimings();
      errorNodeId = undefined;
      viewerComponent?.update(graphResult);
      pm.thumbnails.schedule(g, graphResult);
    } catch (error) {
      // the viewer would keep showing the geometry of the previous graph otherwise
      viewerComponent?.update(new Int32Array());
      // errors from the worker lose everything but their message on the way here
      const info = (await Promise.resolve(runtime.getLastError()).catch(() => undefined))
        ?? getErrorInfo(error);
      errorNodeId = info.nodeId;
      errorLog.add(info);
      toast(info.message, 'error', 6000);
    } finally {
      clearTimeout(timeout);
      isExecuting = false;
      performanceStore.stopRun();
    }
  }

  const handleUpdate = debounceAsyncFunction(update);

  function focusNode(nodeId: number) {
    const node = manager?.getNode(nodeId);
    if (node) graphInterface.state.centerNode(node);
  }

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
  autoStart
  actions={{
    'setup-default': async () => {
      const graph = structuredClone(templates.defaultPlant) as unknown as Graph;
      const existing = pm.projects.find((p) => p.id === tutorialProjectId.value);
      if (!existing) {
        tutorialProjectId.value = pm.handleCreateProject(graph, 'Tutorial Project');
        return;
      }
      graph.id = existing.id;
      graph.meta = { ...existing.meta, lastModified: new Date().toISOString() };
      if (pm.activeId === existing.id && manager) {
        manager.load(graph);
      } else {
        await pm.saveGraph(graph);
        await pm.handleSelectProject(existing.id);
      }
    },
    'load-tutorial-template': () => {
      if (!pm.graph) return;
      const g = structuredClone(templates.tutorial) as unknown as Graph;
      g.id = pm.graph.id;
      g.meta = { ...pm.graph.meta };
      manager.load(g);
      graphInterface.state.centerNode(graphInterface.manager.getAllNodes()[0]);
    },
    'open-github-nodes': () => {
      window.open(
        'https://github.com/jim-fx/nodarium/tree/main/nodes/max/plantarium',
        '_blank',
        'noopener'
      );
    }
  }}
  hooks={{
    'action:add_stem_node': waitForGraph(tutorialChecks.stemToOutput),
    'action:add_noise_node': waitForGraph(tutorialChecks.noiseBetweenStemAndOutput),
    'action:add_random_node': waitForGraph(tutorialChecks.randomOnStem),
    'action:prompt_regenerate': (advance) => {
      onRegenerate = advance;
      return () => (onRegenerate = undefined);
    },
    'before:save_project': () => panelState.setActivePanel(''),
    'before:export_tour': () => panelState.setActivePanel('exports'),
    'before:shortcuts_tour': () => panelState.setActivePanel('shortcuts'),
    'after:save_project': () => panelState.setActivePanel('graph-settings'),
    'before:tour_runtime_nerd': () => panelState.setActivePanel('general')
  }}
/>

<div class="wrapper manager-{manager?.status}">
  <TitleBar
    bind:this={titleBar}
    projectManager={pm}
    saving={pendingSave}
    onopenprojects={() => (projectBrowserOpen = true)}
    onstarttutorial={() => planty?.start()}
  />
  <Grid.Row>
    <Grid.Cell>
      <div class="viewer-cell" data-planty="viewer">
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
            {nodeTimings}
            {errorNodeId}
            onsave={async (g) => {
              pendingSave = true;
              await pm.saveGraph(g);
              pendingSave = false;
            }}
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
          >
            {#snippet customInput(value, setValue)}
              <MaterialPicker {value} onchange={setValue} />
            {/snippet}
          </NestedSettings>
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
          id="errors"
          title="Errors"
          hidden={!appSettings.value.debug.advancedMode}
          icon="i-[tabler--alert-triangle] bg-red-400"
        >
          <ErrorLog onfocusnode={focusNode} />
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
            {#key activeNode?.exposed?.join()}
              <ActiveNodeSettings {manager} bind:node={activeNode} />
            {/key}
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

<ProjectBrowser projectManager={pm} bind:open={projectBrowserOpen} />

<Toast />

<style>
  .wrapper {
    height: 100vh;
    width: 100vw;
    color: white;
    display: grid;
    grid-template-rows: 36px minmax(0, 1fr);
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
