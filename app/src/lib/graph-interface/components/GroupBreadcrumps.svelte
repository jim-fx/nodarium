<script lang="ts">
  import { getGraphManager } from '../graph-state.svelte';
  const graph = getGraphManager();

  function getGroupName(groupId: number) {
    const group = graph.getGroup(groupId);
    return group?.name || `Group#${groupId}`;
  }

  function exitToGroup(groupId?: number) {
    while (graph.graphStack.length > 0 && graph.graphStack.at(-1)?.groupId !== groupId) {
      graph.exitGroup();
    }
  }
</script>

<div class="shadow" class:is-inside-group={graph.isInsideGroup}></div>

{#if graph.isInsideGroup}
  <div class="group-name flex gap-1 items-center">
    <button
      class="bg-layer-2 opacity-75 hover:opacity-100 cursor-pointer rounded-sm p-1 px-2"
      onclick={() => exitToGroup()}
    >
      Root
    </button>
    {#each graph.graphStack as group (group.groupId)}
      <span class="i-[tabler--arrow-right]"></span>
      <button
        class="bg-layer-2 opacity-75 hover:opacity-100 cursor-pointer rounded-sm p-1 px-2"
        onclick={() => exitToGroup(group.groupId)}
      >
        {getGroupName(group.groupId)}
      </button>
    {/each}
  </div>
{/if}

<style>
  .shadow {
    position: absolute;
    top: -5px;
    left: -5px;
    right: calc(var(--padding-right) - 5px);
    bottom: -5px;
    z-index: 1;
    transition: box-shadow 0.3s ease;
    box-shadow: 0 0 0px 0px var(--color-layer-2) inset;
    pointer-events: none;
  }

  .shadow.is-inside-group  {
    box-shadow: 0 0 0px 8px var(--color-layer-2) inset;
  }

  .group-name {
    position: absolute;
    left: calc(50% - var(--padding-right) / 2);
    transition: left 0.3s ease;
    top: 12px;
    transform: translateX(-50%);
    z-index: 1000;
  }
</style>
