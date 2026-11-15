<script lang="ts">
  import type { GraphManager } from '../graph-manager.svelte';
  import { GraphState, setGraphManager, setGraphState } from '../graph-state.svelte';
  import NodeHTML from './NodeHTML.svelte';

  // The nodes of a graph as Graph.svelte shows them, without its canvas. For tests.
  const { manager }: { manager: GraphManager } = $props();

  // svelte-ignore state_referenced_locally
  setGraphManager(manager);
  // svelte-ignore state_referenced_locally
  setGraphState(new GraphState(manager));
</script>

{#each manager.nodeArray as node, index (node)}
  <NodeHTML bind:node={manager.nodeArray[index]} />
{/each}
