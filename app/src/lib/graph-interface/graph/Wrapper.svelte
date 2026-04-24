<script lang="ts">
  import { createKeyMap } from '$lib/helpers/createKeyMap';
  import type { Graph, NodeInstance, NodeRegistry } from '@nodarium/types';
  import { GraphManager } from '../graph-manager.svelte';
  import { GraphState, setGraphManager, setGraphState } from '../graph-state.svelte';
  import { setupKeymaps } from '../keymaps';
  import GraphEl from './Graph.svelte';

  type Props = {
    graph?: Graph;
    registry: NodeRegistry;

    settings?: Record<string, unknown>;

    activeNode?: NodeInstance;
    backgroundType?: 'grid' | 'dots' | 'none';
    snapToGrid?: boolean;
    showHelp?: boolean;
    settingTypes?: Record<string, unknown>;

    safePadding?: { left?: number; right?: number; bottom?: number; top?: number };

    onsave?: (save: Graph) => void;
    onresult?: (result: unknown) => void;
  };

  let {
    graph,
    registry,
    safePadding,
    settings = $bindable(),
    activeNode = $bindable(),
    backgroundType = $bindable('grid'),
    snapToGrid = $bindable(true),
    showHelp = $bindable(false),
    settingTypes = $bindable(),
    onsave,
    onresult
  }: Props = $props();

  export const keymap = createKeyMap([]);

  // svelte-ignore state_referenced_locally
  export const manager = new GraphManager(registry);
  setGraphManager(manager);

  export const state = new GraphState(manager);
  $effect(() => {
    if (safePadding) {
      state.safePadding = safePadding;
    }
    state.backgroundType = backgroundType;
    state.snapToGrid = snapToGrid;
    state.showHelp = showHelp;
  });

  setGraphState(state);

  setupKeymaps(keymap, manager, state);

  $effect(() => {
    if (state.activeNodeId !== -1) {
      activeNode = manager.getNode(state.activeNodeId);
    } else if (activeNode) {
      activeNode = undefined;
    }
  });

  $effect(() => {
    if (!state.addMenuPosition) {
      state.edgeEndPosition = null;
      state.activeSocket = null;
    }
  });

  manager.on('settings', (_settings) => {
    settingTypes = { ...settingTypes, ..._settings.types };
    settings = _settings.values;
  });

  manager.on('result', (result) => onresult?.(result));

  manager.on('save', (save) => onsave?.(save));

  $effect(() => {
    if (graph) {
      manager.load(graph);
    }
  });

  function navigateToBreadcrumb(index: number) {
    const crumbs = manager.breadcrumbs;
    const depth = crumbs.length - 1 - index;
    let result: { camera: [number, number, number]; nodeId: number } | false = false;
    for (let i = 0; i < depth; i++) {
      const groupId = manager.currentGroupContext;
      if (groupId) {
        state.groupCameras.set(groupId, [...state.cameraPosition] as [number, number, number]);
      }
      result = manager.exitGroup();
    }
    if (result !== false) {
      state.activeNodeId = result.nodeId;
      state.clearSelection();
      state.cameraPosition[0] = result.camera[0];
      state.cameraPosition[1] = result.camera[1];
      state.cameraPosition[2] = result.camera[2];
    } else {
      state.activeNodeId = -1;
      state.clearSelection();
      state.centerNode();
    }
  }
</script>

{#if manager.isInsideGroup}
  <div class="breadcrumb-bar">
    {#each manager.breadcrumbs as crumb, i}
      {#if i > 0}
        <span class="sep">›</span>
      {/if}
      <button
        class="crumb"
        class:active={i === manager.breadcrumbs.length - 1}
        onclick={() => navigateToBreadcrumb(i)}
      >
        {crumb.name}
      </button>
    {/each}
  </div>
{/if}

<GraphEl {keymap} {safePadding} />

<style>
  .breadcrumb-bar {
    position: absolute;
    top: 8px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 100;
    display: flex;
    align-items: center;
    gap: 4px;
    background: rgba(10, 15, 28, 0.85);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 6px;
    padding: 4px 10px;
    font-size: 12px;
    pointer-events: all;
    backdrop-filter: blur(8px);
  }

  .sep {
    opacity: 0.4;
    font-size: 14px;
  }

  .crumb {
    background: none;
    border: none;
    color: rgba(255, 255, 255, 0.6);
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 4px;
    font-size: 12px;
    transition: color 0.15s, background 0.15s;
  }

  .crumb:hover {
    color: white;
    background: rgba(255, 255, 255, 0.08);
  }

  .crumb.active {
    color: white;
    cursor: default;
  }

  .crumb.active:hover {
    background: none;
  }
</style>
