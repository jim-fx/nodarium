<script lang="ts">
  import type { GraphManager } from '../graph-manager.svelte';
  import { GraphState, setGraphManager, setGraphState } from '../graph-state.svelte';
  import type { InputBadge, InputContextMenuEvent } from '../options';
  import { createGraphOptions } from '../test-utils';
  import NodeHTML from './NodeHTML.svelte';

  // The nodes of a graph as Graph.svelte shows them, without its canvas. For tests.
  const {
    manager,
    inputBadge,
    oninputcontextmenu
  }: {
    manager: GraphManager;
    inputBadge?: InputBadge;
    oninputcontextmenu?: (event: InputContextMenuEvent) => void;
  } = $props();

  // svelte-ignore state_referenced_locally
  setGraphManager(manager);
  const options = createGraphOptions();
  // svelte-ignore state_referenced_locally
  const state = new GraphState(manager, () => options);
  state.readInputBadge = () => inputBadge;
  state.readInputContextMenu = () => oninputcontextmenu;
  setGraphState(state);
</script>

{#each manager.nodeArray as node, index (node)}
  <NodeHTML bind:node={manager.nodeArray[index]} />
{/each}
