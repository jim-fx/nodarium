<script lang="ts">
  import type { GraphManager } from '$lib/graph-interface/graph-manager.svelte';
  import type { NodeInstance } from '@nodarium/types';
  import ActiveNodeSelected from './ActiveNodeSelected.svelte';

  type Props = {
    manager: GraphManager;
    node: NodeInstance | undefined;
  };

  let { manager, node = $bindable() }: Props = $props();

  const isGroupInstance = $derived(node?.type === '__internal/group/instance');
  const activeGroup = $derived(
    isGroupInstance ? manager.getGroup(node!.props?.groupId as number) : undefined
  );

  const groupName = $derived(activeGroup?.name ?? '');

  function handleRename(e: Event) {
    const name = (e.target as HTMLInputElement).value;
    if (activeGroup) manager.renameGroup(activeGroup.id, name);
  }
</script>

<div class='{node?"border-l-2 pl-3.5!":""} bg-layer-2 flex items-center h-[70px] border-b-1 border-l-selected border-b-outline pl-4'>
  <h3>Node Settings</h3>
</div>

{#if node}
  {#key node.id}
    {#if isGroupInstance && activeGroup}
      <div class="group-settings">
        <label for="group-name">Group name</label>
        <input
          id="group-name"
          type="text"
          placeholder="Group {activeGroup.id}"
          value={groupName}
          oninput={handleRename}
        />
      </div>
    {:else if node}
      <ActiveNodeSelected {manager} bind:node />
    {/if}
  {/key}
{:else}
  <p class="mx-4 mt-4">No node selected</p>
{/if}

{#if manager?.graph.groups.length}
  <div class="group-actions">
    <button onclick={() => manager.removeUnusedGroups()}>
      Remove unused groups
    </button>
  </div>
{/if}

<style>
  .group-settings {
    display: flex;
    flex-direction: column;
    gap: 0.4em;
    padding: 1em;
  }

  .group-settings label {
    font-size: 0.8em;
    opacity: 0.7;
  }

  .group-settings input {
    background: var(--color-layer-1);
    border: 1px solid var(--color-outline);
    border-radius: 4px;
    color: var(--color-text);
    font-family: var(--font-family);
    font-size: 0.9em;
    padding: 0.4em 0.6em;
  }

  .group-settings input:focus {
    outline: 1px solid var(--color-active);
  }

  .group-actions {
    padding: 0.75em 1em;
    border-top: 1px solid var(--color-outline);
    margin-top: auto;
  }

  .group-actions button {
    width: 100%;
    padding: 0.4em 0.8em;
    background: var(--color-layer-1);
    border: 1px solid var(--color-outline);
    border-radius: 4px;
    color: var(--color-text);
    font-family: var(--font-family);
    font-size: 0.85em;
    cursor: pointer;
  }

  .group-actions button:hover {
    border-color: var(--color-active);
  }
</style>
