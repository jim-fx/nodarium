<script lang="ts">
  import type { AgentApiHost, BuildRecord } from '#lib/agent-api/agent-api.ts';
  import GraphInterface, { type GraphOptions } from '#lib/graph-interface/index.ts';
  import * as templates from '#lib/graph-templates/index.ts';
  import Grid from '#lib/grid/index.ts';
  import { debounceAsyncFunction } from '#lib/helpers/index.ts';
  import { localState } from '#lib/helpers/localState.svelte.ts';
  import { groupNode } from '#lib/node-registry/groupNode.ts';
  import { createAppRegistry } from '#lib/node-registry/index.ts';
  import { colors } from '#lib/theme/colors.svelte.ts';

  import PerformanceViewer from '#lib/performance/PerformanceViewer.svelte';
  import { createRepeatGuard } from '#lib/performance/run-values.ts';
  import { openUrlProject } from '#lib/project-manager/open-url.ts';
  import { ProjectManager } from '#lib/project-manager/project-manager.svelte.ts';
  import ProjectBrowser from '#lib/project-manager/ProjectBrowser.svelte';
  import { type ProbePath, readPaths } from '#lib/result-viewer/gizmos/gizmo-paths.ts';
  import {
    addProbe,
    collectGizmos,
    type GizmoChange,
    probeSource
  } from '#lib/result-viewer/gizmos/gizmos.ts';
  import ResultStats from '#lib/result-viewer/ResultStats.svelte';
  import Viewer from '#lib/result-viewer/Viewer.svelte';
  import { createDragHold } from '#lib/runtime/drag-hold.ts';
  import { errorLog } from '#lib/runtime/error-log.svelte.ts';
  import { WorkerRuntimeExecutor } from '#lib/runtime/index.ts';
  import { errorInfo } from '#lib/runtime/run-error.ts';
  import type { AppRuntime } from '#lib/runtime/types.ts';
  import { appSettings, AppSettingTypes } from '#lib/settings/app-settings.svelte.ts';
  import type { SettingsValue } from '#lib/settings/index.ts';
  import MaterialPicker from '#lib/settings/MaterialPicker.svelte';
  import NestedSettings from '#lib/settings/NestedSettings.svelte';
  import Panel from '#lib/sidebar/Panel.svelte';
  import ActiveNodeSettings from '#lib/sidebar/panels/ActiveNodeSettings.svelte';
  import BenchmarkPanel from '#lib/sidebar/panels/BenchmarkPanel.svelte';
  import Changelog from '#lib/sidebar/panels/Changelog.svelte';
  import ErrorLog from '#lib/sidebar/panels/ErrorLog.svelte';
  import ExportSettings from '#lib/sidebar/panels/ExportSettings.svelte';
  import GraphSource from '#lib/sidebar/panels/GraphSource.svelte';
  import GroupSettings from '#lib/sidebar/panels/GroupSettings.svelte';
  import Keymap from '#lib/sidebar/panels/Keymap.svelte';
  import TemplatePanel from '#lib/sidebar/panels/TemplatePanel.svelte';
  import { panelState } from '#lib/sidebar/PanelState.svelte.ts';
  import Sidebar from '#lib/sidebar/Sidebar.svelte';
  import TitleBar from '#lib/title-bar/TitleBar.svelte';
  import { tutorialChecks } from '#lib/tutorial/tutorial-checks.ts';
  import { tutorialConfig } from '#lib/tutorial/tutorial-config.ts';
  import { readUrlOptions, withoutUrlOptions } from '#lib/url-options.ts';
  import { goto } from '$app/navigation';
  import { resolve } from '$app/paths';
  import { page } from '$app/state';
  import { Planty, type StepActionHook } from '@nodarium/planty';
  import { debugNode, MemoryRuntimeCache, MemoryRuntimeExecutor } from '@nodarium/runtime';
  import { loadTemplate } from '@nodarium/runtime/templates';
  import type { Edge, Graph, NodeDefinition } from '@nodarium/types';
  import { createKeyMap, Spinner, Toast, toast, unitSettings } from '@nodarium/ui';
  import { createPerformanceStore } from '@nodarium/utils';
  import { onDestroy, onMount, untrack } from 'svelte';
  import type { Group } from 'three';
  import { version } from '../../package.json';

  const urlOptions = readUrlOptions(new URL(location.href));

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

  const nodeRegistry = createAppRegistry([debugNode, groupNode]);
  const registryCache = nodeRegistry.cache;
  const workerRuntime = new WorkerRuntimeExecutor();
  const runtimeCache = new MemoryRuntimeCache();
  const memoryRuntime = new MemoryRuntimeExecutor(nodeRegistry, runtimeCache);
  memoryRuntime.perf = performanceStore;
  const pm = new ProjectManager(nodeRegistry, (pm) => openUrlProject(pm, urlOptions));

  function removeUrlOptions() {
    const clean = withoutUrlOptions(new URL(location.href));
    if (clean !== undefined) {
      const path = resolve(`/${clean}`);
      void goto(path, { replace: true, shallow: true, state: page.state });
    }
  }

  // a link pasted into a tab that has the app open only changes the fragment, the page stays
  function handleHashChange() {
    const { graph } = readUrlOptions(new URL(location.href));
    if (graph === undefined) return;
    removeUrlOptions();
    void openUrlProject(pm, { tutorial: true, graph });
  }

  onMount(() => {
    removeUrlOptions();
    const panel = urlOptions.panel && panelState.panels[urlOptions.panel];
    if (panel && !panel.hidden) panelState.setActivePanel(urlOptions.panel!);
  });

  const runtime: AppRuntime = $derived(
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

  // the editor only loads definitions, so the worker would fetch the wasm only on the first run
  $effect(() => {
    const graphId = pm.graph?.id;
    if (graphId === undefined || !appSettings.value.debug.useWorker) return;
    untrack(() => {
      const graph = pm.graph!;
      const types = [...graph.nodes, ...(graph.groups ?? []).flatMap((g) => g.nodes)]
        .filter((n) => n && 'type' in n)
        .map((n) => n.type);
      workerRuntime.loadNodes(types);
    });
  });

  // the worker and the registries of the thumbnails and the garden can't fetch a dropped node
  async function registerDroppedNode(node: NodeDefinition, wasm: ArrayBuffer) {
    // the worker gets its own copy: a transfer would leave the one that is kept empty
    await Promise.all([
      workerRuntime.registerNode(wasm.slice(0)),
      nodeRegistry.keepDropped(node, wasm)
    ]);
  }

  function handleBeforeUnload(e: BeforeUnloadEvent) {
    if (pendingSave) e.preventDefault();
  }

  let scene = $state<Group>(null!);
  let isExecuting = $state(false);

  let sidebarOpen = $state(false);
  let graphInterface = $state<ReturnType<typeof GraphInterface>>(null!);
  let viewerComponent = $state<ReturnType<typeof Viewer>>();
  let debugData = $state<Record<number, { type: string; data: Int32Array }>>();
  let nodeTimings = $state<Record<number, number>>({});
  let errorNodeId = $state<number>();
  let lastBuild = $state.raw<BuildRecord>();
  /** Builds that were asked for and are not finished, the running one and the one waiting */
  let buildsRunning = $state(0);
  const safePadding = $derived({ right: sidebarOpen ? 321 : undefined });
  // read one by one: a run that only brings new timings leaves what reads the theme alone
  const graphOptions: GraphOptions = {
    get backgroundType() {
      return appSettings.value.nodeInterface.backgroundType;
    },
    get snapToGrid() {
      return appSettings.value.nodeInterface.snapToGrid;
    },
    get showHelp() {
      return appSettings.value.nodeInterface.showHelp;
    },
    get showDebugInfo() {
      return appSettings.value.debug.advancedMode;
    },
    get theme() {
      return appSettings.value.theme;
    },
    colors,
    get safePadding() {
      return safePadding;
    },
    get nodeTimings() {
      return nodeTimings;
    },
    get errorNodeId() {
      return errorNodeId;
    }
  };
  const manager = $derived(graphInterface?.manager);
  const activeNode = $derived(
    graphInterface?.state.activeNodeId !== undefined && graphInterface.state.activeNodeId !== -1
      ? manager?.getNode(graphInterface.state.activeNodeId)
      : undefined
  );

  /** The paths that come into the active node, for the gizmos of nodes that work on paths */
  let probe = $state.raw<{ sourceId: number; paths: ProbePath[] }>();
  const probeSourceOfActive = $derived(
    manager && activeNode ? probeSource(activeNode.type, activeNode.id, manager.edges) : undefined
  );

  const gizmos = $derived(
    manager
      ? collectGizmos(
        manager.nodes.values(),
        manager.edges,
        graphInterface.state.activeNodeId,
        (node, input) => {
          const definition = node.state?.type?.inputs?.[input];
          return definition && 'value' in definition ? definition.value : undefined;
        },
        probe && probe.sourceId === probeSourceOfActive?.nodeId ? probe.paths : []
      )
      : []
  );

  // selecting a node that needs the paths coming in runs the graph, they are collected on the way
  const probeKey = $derived(
    probeSourceOfActive && activeNode
      ? `${activeNode.id}:${probeSourceOfActive.nodeId}:${probeSourceOfActive.output}`
      : undefined
  );
  $effect(() => {
    if (probeKey !== undefined) untrack(() => manager?.execute());
  });

  function handleGizmoChange(nodeId: number, change: GizmoChange) {
    const node = manager?.getNode(nodeId);
    if (!node) return;
    node.props = { ...node.props, ...change };
    manager.commit();
  }

  pm.onrenameactive = (title) => {
    if (manager) manager.meta = { ...manager.meta, title };
  };
  pm.onunitactive = (unit) => {
    if (manager) manager.meta = { ...manager.meta, unit };
  };

  // number inputs and the viewer show lengths in the unit of the project
  $effect(() => {
    unitSettings.unit = manager?.meta?.unit ?? 'm';
    unitSettings.show = appSettings.value.showUnits;
  });

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
    manager.setSetting('seed', Math.floor(Math.random() * 100000000));
    // the benchmark waits for this build
    await build(manager.serialize());
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
      callback: () => pm.handleCreateProject()
    },
    {
      key: 'F2',
      preventDefault: true,
      description: 'Rename project',
      callback: () => titleBar?.startRename()
    }
  ]);

  let timeout: ReturnType<typeof setTimeout>;
  const runs = createRepeatGuard();

  /** The timings of a run in the worker join those of the page, the rest of the wait is transfer */
  function mergeWorkerRun({ total, ...points }: Record<string, number[]>, waited: number) {
    if (!total) return;
    performanceStore.mergeData({ ...points, runtime: total });
    performanceStore.addPoint('worker-transfer', waited - total[0]);
  }

  async function update(g: Graph) {
    const probed = addProbe(g, graphInterface?.state.activeNodeId ?? -1);
    // loading a graph or pressing R asks for the same run twice, the second would only
    // read the cache and push the time of the real build out of the performance viewer
    const { nodes, edges, groups, settings = {} } = probed.graph;
    const runKey = JSON.stringify([nodes, edges, groups, settings]);
    if (runs.isRepeat(runKey)) return;

    if (timeout) clearTimeout(timeout);
    timeout = setTimeout(() => {
      isExecuting = true;
    }, 100);
    performanceStore.startRun();
    const startedAt = performance.now();
    try {
      const run = await runtime.run(probed.graph, settings);
      const waited = performance.now() - startedAt;
      if (run.performance) mergeWorkerRun(run.performance, waited);

      probe = probed.sourceId === undefined
        ? undefined
        : { sourceId: probed.sourceId, paths: readPaths(run.debugData[probed.sourceId]?.data) };
      // the output of a node that is only watched for the gizmo is not drawn
      debugData = probed.hidden
        ? Object.fromEntries(
          Object.entries(run.debugData).filter(([id]) => Number(id) !== probed.sourceId)
        )
        : run.debugData;
      nodeTimings = run.nodeTimings;
      dragHold.measured(g, run.runTimings);
      errorNodeId = undefined;
      const shown = viewerComponent?.update(run.result);
      lastBuild = {
        graph: g,
        result: run.result,
        vertices: shown?.vertices ?? 0,
        faces: shown?.faces ?? 0,
        bounds: shown?.bounds,
        ms: waited,
        nodeTimings: run.nodeTimings
      };
      pm.thumbnails.schedule(g, run.result);
      runs.succeeded(runKey);
    } catch (error) {
      runs.failed();
      // the viewer would keep showing the geometry of the previous graph otherwise
      viewerComponent?.update(new Int32Array());
      const info = errorInfo(error);
      lastBuild = {
        graph: g,
        result: new Int32Array(),
        error: info,
        vertices: 0,
        faces: 0,
        ms: performance.now() - startedAt,
        nodeTimings: {}
      };
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

  async function build(g: Graph) {
    buildsRunning++;
    try {
      await handleUpdate(g);
    } finally {
      buildsRunning--;
    }
  }

  // a build can't be stopped, on a slow graph one of an early value of a drag delays the last one
  const dragHold = createDragHold({
    build,
    timings: () => nodeTimings,
    inputs: (type) => nodeRegistry.getNode(type)?.inputs
  });
  $effect(() => dragHold.listen(window));

  // dragHold.holding is no $state, but it only changes together with buildPending or buildsRunning
  function isIdle() {
    const m = manager;
    if (!m || pm.opening || buildsRunning > 0 || m.buildPending || dragHold.holding) return false;
    if (m.status === 'error') return true;
    return m.status === 'idle' && pm.graph?.id === m.id;
  }

  const agentApiHost: AgentApiHost = {
    app: {
      version,
      get sha() {
        return data.git?.sha ?? '';
      }
    },
    manager: () => manager,
    isIdle,
    lastBuild: () => lastBuild,
    build,
    importGraph: (input, options) => pm.importGraph(input, options),
    createProject: (graph, title) => pm.handleCreateProject(graph, title),
    registry: nodeRegistry,
    viewCenter: () => {
      const [x, y] = graphInterface.state.camera.position;
      return [x, y];
    },
    renderViews: (result, size, views) => pm.thumbnails.renderViews(result, size, views)
  };
  // only tools use it, the page doesn't wait for its code
  const agentApi = import('#lib/agent-api/agent-api.ts')
    .then(({ createAgentApi }) => window.nodarium = createAgentApi(agentApiHost))
    .catch(() => undefined);
  onDestroy(() => {
    agentApi.then((api) => {
      if (api && window.nodarium === api) delete window.nodarium;
    });
  });

  function focusNode(nodeId: number) {
    const node = manager?.getNode(nodeId);
    if (node) graphInterface.state.centerNode(node);
  }

  function handleSettingsButton(id: string) {
    switch (id) {
      case 'general.planty':
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
<svelte:window onbeforeunload={handleBeforeUnload} onhashchange={handleHashChange} />

<Planty
  bind:this={planty}
  config={tutorialConfig}
  autoStart={urlOptions.tutorial}
  actions={{
    'setup-default': async () => {
      const graph = structuredClone(await loadTemplate('default-plant'));
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
      graphInterface.state.centerNode(graphInterface.manager.nodeArray[0]);
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
    git={data.git}
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
          {gizmos}
          ongizmochange={handleGizmoChange}
          ongizmostart={() => manager?.beginUndoStep()}
          ongizmoend={() => manager?.endUndoStep()}
          centerCamera={appSettings.value.centerCamera}
          projectId={pm.activeId}
        />
        {#if lastBuild}
          <ResultStats
            vertices={lastBuild.vertices}
            faces={lastBuild.faces}
            ms={lastBuild.ms}
            class="absolute bottom-4 left-4"
          />
        {/if}
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
            options={graphOptions}
            onhelp={() => panelState.setActivePanel('shortcuts')}
            onsave={async (g) => {
              pendingSave = true;
              await pm.saveGraph(g);
              pendingSave = false;
            }}
            onresult={(result) => dragHold.request(result)}
            onregister={(node, wasm) => registerDroppedNode(node, wasm)}
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
        {#if import.meta.env.DEV}
          <Panel
            id="push-template"
            title="Push to Template"
            hidden={!appSettings.value.debug.advancedMode}
            icon="i-[tabler--file-export] bg-red-400"
          >
            {#if manager?.status === 'idle'}
              <TemplatePanel graph={manager.serialize()} />
            {/if}
          </Panel>
        {/if}
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
          icon="c-[custom--graph] bg-blue-400"
        >
          <span class="block h-[1px]"></span>
          {#if manager}
            <NestedSettings
              id="graph-settings"
              type={manager.settingTypes}
              value={manager.settings as SettingsValue}
              onchange={(key, value) => manager.setSetting(key, value)}
            />
          {/if}
          {#key activeNode}
            {#key activeNode?.exposed?.join()}
              <ActiveNodeSettings {manager} node={activeNode} />
            {/key}
            <GroupSettings graphState={graphInterface?.state} {manager} node={activeNode} />
          {/key}
        </Panel>
        <Panel
          id="changelog"
          title="Changelog"
          icon="i-[tabler--file-text-spark] bg-green-400"
        >
          <Changelog git={data.git} changelog={data.changelog} roadmap={data.roadmap} />
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
    grid-template-columns: minmax(0, 1fr);
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
