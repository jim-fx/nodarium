<script lang="ts">
  import type { NodeInstance } from '@nodarium/types';
  import { getGraphManager, getGraphState } from '../graph-state.svelte';
  import Edge from './Edge.svelte';

  type Props = { node: NodeInstance; z: number };
  const { node, z }: Props = $props();

  const graphState = getGraphState();
  const graph = getGraphManager();

  const nodeType = $derived(graph.getNodeType(node));

  const inputSocket = $derived(nodeType?.outputs?.[0]);
  const pos1 = $derived(
    graphState.getSocketPosition(node, graph.getMutePassthroughInput(node) ?? '')
  );
  const pos2 = $derived(graphState.getSocketPosition(node, 0));
</script>

<Edge
  x1={pos1[0]}
  y1={pos1[1]}
  x2={pos2[0]}
  y2={pos2[1]}
  inputType={inputSocket}
  outputType={inputSocket}
  z={z}
  id={'muted-' + node.id}
/>
