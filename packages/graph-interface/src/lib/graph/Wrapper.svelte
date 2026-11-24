<script lang="ts">
  import type { Graph, NodeDefinition, NodeRegistry } from '@nodarium/runtime';
  import { createKeyMap } from '@nodarium/ui';
  import { onDestroy, onMount } from 'svelte';
  import { persistCamera } from '../camera.svelte';
  import { GraphManager } from '../graph-manager.svelte';
  import { GraphState, setGraphManager, setGraphState } from '../graph-state.svelte';
  import { setupKeymaps } from '../keymaps';
  import type { GraphOptions, InputBadge, InputContextMenuEvent } from '../options';
  import GraphEl from './Graph.svelte';

  type Props = {
    graph?: Graph;
    registry: NodeRegistry;
    options: GraphOptions;

    onsave?: (save: Graph) => void;
    onresult?: (result: Graph) => void;
    /** a wasm file was dropped, the node of it is placed once this resolves */
    onregister?: (node: NodeDefinition, wasm: ArrayBuffer) => Promise<void> | void;
    /** Nodes to point out, apart from the selection: what made the part hovered in the viewer */
    highlightedNodeIds?: number[];
    /** The node under the pointer changed, `undefined` when there is none */
    onnodehover?: (nodeId: number | undefined) => void;
    /** A short text next to the label of an input, read again when what it reads changes */
    inputBadge?: InputBadge;
    /** A right click on an input; `event.preventDefault()` keeps the add menu closed */
    oninputcontextmenu?: (event: InputContextMenuEvent) => void;
  };

  let {
    graph,
    registry,
    options,
    onsave,
    onresult,
    onregister,
    highlightedNodeIds,
    onnodehover,
    inputBadge,
    oninputcontextmenu
  }: Props = $props();

  export const keymap = createKeyMap([]);

  // svelte-ignore state_referenced_locally
  export const manager = new GraphManager(registry);
  // svelte-ignore state_referenced_locally
  manager.valuesOnly = options.valuesOnly ?? false;
  setGraphManager(manager);

  export const state = new GraphState(manager, () => options);
  setGraphState(state);

  // the graph of a values-only editor is no project, a camera stored under its id would move
  // the camera of the project with that id
  if (!manager.valuesOnly) onDestroy(persistCamera(state.camera, () => manager.id));

  setupKeymaps(keymap, manager, state, manager.valuesOnly);

  const highlighted = $derived(new Set(highlightedNodeIds));
  state.readHighlightedNodes = () => highlighted;
  state.readInputBadge = () => inputBadge;
  state.readInputContextMenu = () => oninputcontextmenu;

  $effect(() => {
    const hovered = state.hoveredNodeId;
    onnodehover?.(hovered === -1 ? undefined : hovered);
  });

  $effect(() => {
    if (!state.addMenuPosition) {
      state.edgeEndPosition = null;
      state.activeSocket = null;
    }
  });

  // a build the closed editor still asks for would show its graph instead of the one opened since
  onDestroy(manager.on('result', (result) => onresult?.(result)));

  manager.on('save', (save) => onsave?.(save));

  manager.onregister = (node, wasm) => onregister?.(node, wasm);

  // after the camera of the graph is restored, which puts it where saved graphs have their nodes
  manager.on('placed', () => setTimeout(() => state.centerNode()));

  onMount(() => {
    if (!graph) return;
    void manager.load(graph).then(() => {
      setTimeout(() => {
        if (manager.valuesOnly) state.centerNode();
        else state.showEndOfGraph();
      });
    });
  });
</script>

<!-- the last change is saved only once the changes rest, which a closed page never gets to -->
<svelte:window onbeforeunload={() => manager.flush()} />
<svelte:document
  onvisibilitychange={() => {
    if (document.visibilityState === 'hidden') manager.flush();
  }}
/>

<GraphEl {keymap} />
