<script lang="ts">
  import { createKeyMap } from '#lib/helpers/createKeyMap.ts';
  import type { Graph, NodeRegistry } from '@nodarium/types';
  import { onMount } from 'svelte';
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
  };

  let {
    graph,
    registry,
    options,
    onsave,
    onresult
  }: Props = $props();

  export const keymap = createKeyMap([]);

  // svelte-ignore state_referenced_locally
  export const manager = new GraphManager(registry);
  setGraphManager(manager);

  export const state = new GraphState(manager, () => options);
  setGraphState(state);

  setupKeymaps(keymap, manager, state);

  $effect(() => {
    if (!state.addMenuPosition) {
      state.edgeEndPosition = null;
      state.activeSocket = null;
    }
  });

  manager.on('result', (result) => onresult?.(result));

  manager.on('save', (save) => onsave?.(save));

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
