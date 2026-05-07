<script lang="ts">
  import { Button } from '@nodarium/ui';
  import { getGraphManager } from '../graph-state.svelte';
  const graph = getGraphManager();

  function getGroupName(groupId: number) {
    const group = graph.getGroup(groupId);
    return group?.name || `Group#${groupId}`;
  }

  function exitToGroup(targetId?: number) {
    while (graph.currentGroupId !== (targetId ?? null)) {
      graph.exitGroup();
    }
  }

  // Intermediate groups: parent stack entries that are groups (not the root graph).
  const intermediateGroups = $derived(
    graph.parentStack.filter(e => e.id !== graph.id)
  );
</script>

<div class="shadow" class:is-inside-group={graph.isInsideGroup}></div>

{#if graph.isInsideGroup}
  <div class="group-name flex gap-1 items-center">
    <Button variant="ghost" size="sm" onclick={() => exitToGroup()}>Root</Button>

    {#each intermediateGroups as entry (entry.id)}
      <span class="i-[tabler--arrow-right]"></span>
      <Button variant="ghost" size="sm" onclick={() => exitToGroup(entry.id)}>
        {getGroupName(entry.id)}
      </Button>
    {/each}

    <span class="i-[tabler--arrow-right]"></span>
    <Button variant="ghost" size="sm" class="opacity-100!">
      {getGroupName(graph.currentGroupId!)}
    </Button>
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
