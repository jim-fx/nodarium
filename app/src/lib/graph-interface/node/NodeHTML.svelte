<script lang="ts">
  import type { NodeDefinition, NodeInstance } from '@nodarium/types';
  import { getGraphManager, getGraphState } from '../graph-state.svelte';
  import NodeHeader from './NodeHeader.svelte';
  import NodeParameter from './NodeParameter.svelte';

  let ref: HTMLDivElement;

  const graphState = getGraphState();
  const manager = getGraphManager();

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

  function buildParameters(node: NodeInstance, inputs: NodeDefinition['inputs']) {
    let parameters = Object.entries(inputs || {}).filter(
      (p) => p[1].type !== 'seed' && !('setting' in p[1]) && p[1]?.hidden !== true
    );

    if (node.type === '__virtual/group/instance') {
      const groupOptions = [...(manager?.groups?.entries() ?? [])].map(([id, g]) => ({
        label: g.name,
        value: id
      }));
      // Remove the static placeholder from the definition (height-only) and replace
      // with a fully dynamic version that carries current names + value.
      parameters = parameters.filter(([key]) => key !== '__virtual/groupId');
      parameters = [['__virtual/groupId', {
        type: 'select',
        value: node.props?.groupId as string,
        options: groupOptions
      }], ...parameters];
    }

    return parameters;
  }

  $effect(() => {
    const props = node.props as Record<string, unknown> | undefined;
    const virtualGroupId = props?.['__virtual/groupId'] as string | undefined;
    if (!virtualGroupId) return;
    const activeGroupId = props?.groupId as string | undefined;
    if (virtualGroupId === activeGroupId) return;
    const newGroupDef = manager?.groupNodeDefinitions.get(`__virtual/group/${virtualGroupId}`);
    if (!newGroupDef) return;
    const { children, parents, ref } = node.state;
    node.props = { ...props, groupId: virtualGroupId, '__virtual/groupId': virtualGroupId };
    node.state = { type: newGroupDef, children, parents, ref };
    manager?.execute();
    manager?.save();
  });

  const parameters = $derived(buildParameters(node, node?.state?.type?.inputs || {}));

  const currentGroupId = $derived((node.props?.groupId as string) ?? '');

  function onGroupSelect(event: Event) {
    const select = event.target as HTMLSelectElement;
    const newGroupId = select.value;
    if (!manager || newGroupId === currentGroupId) return;
    const newGroupDef = manager.groupNodeDefinitions.get(`__virtual/group/${newGroupId}`);
    if (!newGroupDef) return;
    node.props = { ...(node.props ?? {}), groupId: newGroupId };
    node.state = { type: newGroupDef };
    manager.execute();
    manager.save();
  }

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

  {#if false && node.type === '__virtual/group/instance'}
    <div class="group-param">
      <select
        value={currentGroupId}
        onchange={onGroupSelect}
        onmousedown={(e) => e.stopPropagation()}
        onclick={(e) => e.stopPropagation()}
        onkeydown={(e) => e.stopPropagation()}
      >
        {#each manager?.groups?.entries() ?? [] as [gid, gdef]}
          <option value={gid}>{gdef.name}</option>
        {/each}
      </select>
    </div>
  {/if}

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
  .group-param {
    padding: 5px 8px;
    border-bottom: solid 1px var(--color-layer-2);
    background: var(--color-layer-1);
  }

  .group-param select {
    width: 100%;
    background: var(--color-layer-2);
    color: var(--color-text);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 4px;
    padding: 4px 6px;
    font-size: 0.8em;
    cursor: pointer;
    box-sizing: border-box;
  }

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
