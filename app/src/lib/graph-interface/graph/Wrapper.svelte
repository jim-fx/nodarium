<script lang="ts">
  import { createKeyMap } from '$lib/helpers/createKeyMap';
  import type { Graph, NodeInstance, NodeRegistry } from '@nodarium/types';
  import { onMount } from 'svelte';
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
    // eslint-disable-next-line no-useless-assignment
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

  onMount(() => {
    if (graph) {
      manager.load(graph);
    }
  });
</script>

<GraphEl {keymap} {safePadding} />
