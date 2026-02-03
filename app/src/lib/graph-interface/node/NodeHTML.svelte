<script lang="ts">
  import type { NodeInstance } from '@nodarium/types';
  import { getGraphState } from '../graph-state.svelte';
  import NodeHeader from './NodeHeader.svelte';
  import NodeParameter from './NodeParameter.svelte';

  let ref: HTMLDivElement;

  const graphState = getGraphState();

  type Props = {
    node: NodeInstance;
    position?: 'absolute' | 'fixed' | 'relative';
    isActive?: boolean;
    isSelected?: boolean;
    inView?: boolean;
    z?: number;
  };

  let {
    node = $bindable(),
    position = 'absolute',
    isActive = false,
    isSelected = false,
    inView = true,
    z = 2
  }: Props = $props();

  // If we dont have a random offset, all nodes becom visible at the same zoom level -> stuttering
  const zOffset = Math.random() - 0.5;
  const zLimit = 2 - zOffset;

  const parameters = Object.entries(node.state?.type?.inputs || {}).filter(
    (p) => p[1].type !== 'seed' && !('setting' in p[1]) && p[1]?.hidden !== true
  );

  $effect(() => {
    if ('state' in node && !node.state.ref) {
      node.state.ref = ref;
      graphState?.updateNodePosition(node);
    }
  });
</script>

<div
  class="node {position}"
  class:active={isActive}
  style:--cz={z + zOffset}
  style:display={inView && z > zLimit ? 'block' : 'none'}
  class:selected={isSelected}
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

  .node.active {
    --stroke: var(--color-active);
    --stroke-width: 2px;
  }

  .node.selected {
    --stroke: var(--color-selected);
    --stroke-width: 2px;
  }
</style>
