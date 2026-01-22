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

    addMenuPadding?: { left?: number; right?: number; bottom?: number; top?: number };

    onsave?: (save: Graph) => void;
    onresult?: (result: unknown) => void;
  };

  let {
    graph,
    registry,
    addMenuPadding,
    settings = $bindable(),
    activeNode = $bindable(),
    backgroundType = $bindable('grid'),
    snapToGrid = $bindable(true),
    showHelp = $bindable(false),
    settings = $bindable(),
    settingTypes = $bindable(),
    onsave,
    onresult
  }: Props = $props();

  export const keymap = createKeyMap([]);

  // svelte-ignore state_referenced_locally
  export const manager = new GraphManager(registry);
  setGraphManager(manager);

  const graphState = new GraphState(manager);
  $effect(() => {
    graphState.backgroundType = backgroundType;
    graphState.snapToGrid = snapToGrid;
    graphState.showHelp = showHelp;
  });

  setGraphState(graphState);

  setupKeymaps(keymap, manager, graphState);

  $effect(() => {
    if (graphState.activeNodeId !== -1) {
      activeNode = manager.getNode(graphState.activeNodeId);
    } else if (activeNode) {
      activeNode = undefined;
    }
  });

  $effect(() => {
    if (!graphState.addMenuPosition) {
      graphState.edgeEndPosition = null;
      graphState.activeSocket = null;
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
</script>

<GraphEl {keymap} {addMenuPadding} />
