<script lang="ts">
  import type { Graph, NodeDefinition, NodeRegistry } from '@nodarium/types';
  import { createKeyMap } from '@nodarium/ui';
  import { onDestroy, onMount } from 'svelte';
  import { persistCamera } from '../camera.svelte';
  import { GraphManager } from '../graph-manager.svelte';
  import { GraphState, setGraphManager, setGraphState } from '../graph-state.svelte';
  import { setupKeymaps } from '../keymaps';
  import type { GraphOptions } from '../options';
  import GraphEl from './Graph.svelte';

  type Props = {
    graph?: Graph;
    registry: NodeRegistry;
    options: GraphOptions;

    onsave?: (save: Graph) => void;
    onresult?: (result: Graph) => void;
    /** a wasm file was dropped, the node of it is placed once this resolves */
    onregister?: (node: NodeDefinition, wasm: ArrayBuffer) => Promise<void> | void;
    /** the `?` shortcut */
    onhelp?: () => void;
  };

  let {
    graph,
    registry,
    options,
    onsave,
    onresult,
    onregister,
    onhelp
  }: Props = $props();

  export const keymap = createKeyMap([]);

  // svelte-ignore state_referenced_locally
  export const manager = new GraphManager(registry);
  setGraphManager(manager);

  export const state = new GraphState(manager, () => options);
  setGraphState(state);

  onDestroy(persistCamera(state.camera, () => manager.id));

  setupKeymaps(keymap, manager, state, () => onhelp?.());

  $effect(() => {
    if (!state.addMenuPosition) {
      state.edgeEndPosition = null;
      state.activeSocket = null;
    }
  });

  manager.on('result', (result) => onresult?.(result));

  manager.on('save', (save) => onsave?.(save));

  manager.onregister = (node, wasm) => onregister?.(node, wasm);

  // after the camera of the graph is restored, which puts it where saved graphs have their nodes
  manager.on('placed', () => setTimeout(() => state.centerNode()));

  onMount(() => {
    if (graph) {
      manager.load(graph);
    }
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
