<script lang="ts">
  import type { NodeInstance } from '@nodarium/types';
  import MutedEdge from '../edges/MutedEdge.svelte';
  import { tryGetGraphManager, tryGetGraphState } from '../graph-state.svelte';
  import NodeHeader from './NodeHeader.svelte';
  import NodeParameter from './NodeParameter.svelte';

  let ref: HTMLDivElement;

  const graph = tryGetGraphManager();
  const graphState = tryGetGraphState();

  type Props = {
    node: NodeInstance;
    position?: 'absolute' | 'fixed' | 'relative';
    isActive?: boolean;
    isSelected?: boolean;
    hasError?: boolean;
    inView?: boolean;
    z?: number;
  };

  let {
    node = $bindable(),
    position = 'absolute',
    isActive = false,
    isSelected = false,
    hasError = false,
    inView = true,
    z = 2
  }: Props = $props();

  // If we dont have a random offset, all nodes becom visible at the same zoom level -> stuttering
  const zOffset = Math.random() - 0.5;
  const zLimit = 2 - zOffset;

  const nodeType = $derived(graph.getNodeType(node));

  const parameters = $derived(
    Object.entries(nodeType?.inputs || {}).filter(
      (p) =>
        p[1].type !== 'seed' && p[1].type !== 'creator' && !('setting' in p[1])
        && p[1]?.hidden !== true
    ) || {}
  );

  const showMutedEdge = $derived.by(() => {
    const firstInput = graph?.getMutePassthroughInput(node);
    if (!firstInput) return false;
    if (graph?.getEdgesToNode(node).some((e) => e[3] === firstInput) !== true) return false;
    const inputType = nodeType?.inputs?.[firstInput]?.type;
    const outputType = nodeType?.outputs?.[0];
    return inputType === outputType;
  });

  $effect(() => {
    if ('state' in node && !node.state.ref) {
      node.state.ref = ref;
      graphState?.updateNodePosition(node);
    }
  });
</script>

{#if node.state.muted && node.state.children?.length && showMutedEdge}
  <MutedEdge {node} {z} />
{/if}

<div
  class="node {position}"
  class:active={isActive}
  class:muted={node.state.muted}
  style:--cz={z + zOffset}
  style:display={inView && z > zLimit ? 'block' : 'none'}
  class:selected={isSelected}
  class:error={hasError}
  class:out-of-view={!inView}
  data-node-id={node.id}
  data-node-type={node.type}
  bind:this={ref}
>
  <NodeHeader {node} />

  {#each parameters as [key, value], i (key)}
    <NodeParameter
      bind:node
      id={key}
      input={value}
      isLast={i == parameters.length - 1}
    />
  {/each}
</div>

<style>
  .node {
    box-sizing: border-box;
    user-select: none !important;
    cursor: pointer;
    width: 200px;
    color: var(--color-text);
    transform: translate3d(var(--nx), var(--ny), 0);
    z-index: 1;
    opacity: calc((var(--cz) - 2.5) / 3.5);
    font-weight: 300;
    --stroke: var(--color-outline);
    --stroke-width: 2px;
  }

  .node.muted {
    opacity: 0.5;
  }

  .node.error {
    --stroke: var(--color-error);
    --stroke-width: 2px;
  }

  .node.active {
    --stroke: var(--color-active);
    --stroke-width: 2px;
  }

  .node.selected {
    --stroke: var(--color-selected);
    --stroke-width: 2px;
  }
</style>
