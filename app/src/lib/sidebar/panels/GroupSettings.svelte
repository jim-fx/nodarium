<script lang="ts">
  import type { GraphManager } from '$lib/graph-interface/graph-manager.svelte';
  import type { NodeInstance } from '@nodarium/types';
  import InputSelect from '../../../../../packages/ui/src/lib/inputs/InputSelect.svelte';
  import UnusedGroupsPanel from './UnusedGroupsPanel.svelte';

  type Props = {
    manager: GraphManager;
    node?: NodeInstance;
  };

  const { manager, node = $bindable() }: Props = $props();

  const activeGroup = $derived.by(() => {
    if (manager?.isInsideGroup) {
      const activeGroupId = manager.graphStack?.at(-1)?.groupId;
      if (activeGroupId !== undefined) {
        return manager.getGroup(activeGroupId);
      }
    }

    if (node?.type === '__internal/group/instance') {
      return manager.getGroup(node.props?.groupId as number);
    }
  });

  const groupName = $derived(activeGroup?.name ?? '');
  function handleRename(e: Event) {
    const name = (e.target as HTMLInputElement).value;
    if (activeGroup) manager.renameGroup(activeGroup.id, name);
  }
</script>

{#if activeGroup}
  <div class='{node?"border-l-2 pl-3.5!":""} bg-layer-2 flex items-center h-[70px] border-b-1 border-l-selected border-b-outline pl-4'>
    <h3>Group Settings</h3>
  </div>
{/if}

{#if activeGroup}
  {#key activeGroup.id}
    <div class="group-settings">
      <label for="group-name">Group name</label>
      <input
        id="group-name"
        type="text"
        placeholder="Group {activeGroup.id}"
        value={groupName}
        oninput={handleRename}
      />

      <label for="group-name">Group Inputs</label>
      <div>
        {#each Object.keys(activeGroup?.inputs ?? {}) as key (key)}
          <div class="flex">
            <InputSelect
              value={activeGroup.inputs?.[key].type}
              options={['seed', 'float', 'boolean']}
            />
            <input type="text" placeholder="Input {key}" />
            <button>
              🥊
            </button>
          </div>
        {/each}
      </div>
    </div>
  {/key}
{/if}

{#if manager && !manager.isInsideGroup}
  <UnusedGroupsPanel {manager} />
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
</style>
