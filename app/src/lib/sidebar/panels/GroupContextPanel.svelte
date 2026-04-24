<script lang="ts">
  import type { GraphManager } from '$lib/graph-interface/graph-manager.svelte';
  import { InputSelect } from '@nodarium/ui';

  type Props = { manager: GraphManager; groupId: string };
  const { manager, groupId }: Props = $props();

  const group = $derived(manager.groups.get(groupId));

  const COMMON_TYPES = ['plant', 'float', 'int', 'vec3', 'bool'];
  let selectedTypeIdx = $state(0);
  let customType = $state('');

  function rename(e: Event) {
    if (!group) return;
    const name = (e.target as HTMLInputElement).value.trim();
    if (!name) return;
    group.name = name;
    if (manager.graph.groups?.[groupId]) manager.graph.groups[groupId].name = name;
    const def = manager.groupNodeDefinitions.get(`__virtual/group/${groupId}`);
    if (def?.meta) def.meta.title = name;
    manager.save();
  }

  function addSocket() {
    const type = customType.trim() || COMMON_TYPES[selectedTypeIdx];
    if (!type) return;
    manager.addGroupSocket('input', type);
    customType = '';
  }

  function removeSocket(index: number) {
    manager.removeGroupSocket('input', index);
  }

  function prune() {
    manager.pruneUnusedGroups();
  }
</script>

<div class="bg-layer-2 flex items-center h-[70px] border-b-1 border-outline pl-4">
  <h3>Group Settings</h3>
</div>

<div class="px-4 py-3 flex flex-col gap-4">
  <div class="flex flex-col gap-1.5">
    <span class="section-label">Group name</span>
    <input
      type="text"
      value={group?.name ?? ''}
      onchange={rename}
      onkeydown={(e) => e.stopPropagation()}
      placeholder="Group name"
      class="bg-layer-2 text-text rounded-[5px] px-2 py-1.5 text-sm w-full box-border outline outline-1 outline-outline"
    />
  </div>

  <div class="flex flex-col gap-1.5">
    <span class="section-label">Inputs</span>

    {#if (group?.inputs?.length ?? 0) === 0}
      <p class="text-sm opacity-40 italic m-0">No inputs yet</p>
    {:else}
      <ul class="socket-list">
        {#each group?.inputs ?? [] as socket, i}
          <li class="socket-item">
            <span class="flex-1 opacity-80 text-sm">{socket.name}</span>
            <span class="text-xs opacity-45 italic">{socket.type}</span>
            <button class="remove-btn" onclick={() => removeSocket(i)} title="Remove">×</button>
          </li>
        {/each}
      </ul>
    {/if}

    <div class="flex gap-1.5 items-center">
      <InputSelect options={COMMON_TYPES} bind:value={selectedTypeIdx} />
      <input
        type="text"
        placeholder="custom type…"
        bind:value={customType}
        onkeydown={(e) => {
          e.stopPropagation();
          if (e.key === 'Enter') addSocket();
        }}
        class="bg-layer-2 text-text rounded-[5px] px-2 py-1 text-sm flex-1 min-w-0 outline outline-1 outline-outline"
      />
      <button class="add-btn" onclick={addSocket}>+ Add</button>
    </div>
  </div>

  <div class="flex flex-col gap-1.5">
    <span class="section-label">Maintenance</span>
    <button class="danger-btn" onclick={prune}>Prune unused groups</button>
  </div>
</div>

<style>
  .section-label {
    font-size: 0.72em;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    opacity: 0.5;
  }

  .socket-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .socket-item {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--color-layer-2);
    border-radius: 5px;
    padding: 4px 8px;
    outline: 1px solid var(--color-outline);
  }

  .remove-btn {
    background: none;
    border: none;
    color: var(--color-text);
    cursor: pointer;
    opacity: 0.4;
    padding: 0 2px;
    font-size: 1.1em;
    line-height: 1;
  }

  .remove-btn:hover {
    opacity: 1;
  }

  .add-btn {
    background: var(--color-layer-2);
    color: var(--color-text);
    border: none;
    outline: 1px solid var(--color-outline);
    border-radius: 5px;
    padding: 0.4em 0.7em;
    font-size: 0.8em;
    cursor: pointer;
    white-space: nowrap;
    font-family: var(--font-family);
  }

  .add-btn:hover {
    outline-color: var(--color-selected);
  }

  .danger-btn {
    background: var(--color-layer-2);
    color: var(--color-text);
    border: none;
    outline: 1px solid var(--color-outline);
    border-radius: 5px;
    padding: 0.4em 0.7em;
    font-size: 0.8em;
    cursor: pointer;
    font-family: var(--font-family);
    opacity: 0.7;
    width: 100%;
    text-align: left;
  }

  .danger-btn:hover {
    outline-color: #e05050;
    opacity: 1;
  }
</style>
