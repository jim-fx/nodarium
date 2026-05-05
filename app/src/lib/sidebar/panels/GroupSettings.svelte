<script lang="ts">
  import type { GraphManager } from '$lib/graph-interface/graph-manager.svelte';
  import { GraphState } from '$lib/graph-interface/graph-state.svelte';
  import type { NodeInstance } from '@nodarium/types';
  import { SocketTable } from '@nodarium/ui';
  import UnusedGroupsPanel from './UnusedGroupsPanel.svelte';

  type Props = {
    manager: GraphManager;
    graphState: GraphState;
    node?: NodeInstance;
  };

  const { manager, graphState, node = $bindable() }: Props = $props();

  const activeGroup = $derived.by(() => {
    if (node?.type === '__internal/group/instance') {
      let group = manager.getGroup(node.props?.groupId as number);
      if (group) return group;
    }

    if (manager?.isInsideGroup && manager.currentGroupId !== null) {
      return manager.getGroup(manager.currentGroupId);
    }
  });

  const groupName = $derived(activeGroup?.name ?? '');
  function handleRename(e: Event) {
    const name = (e.target as HTMLInputElement).value;
    if (activeGroup) manager.renameGroup(activeGroup.id, name);
  }

  function handleRemoveInput(key: string) {
    if (!activeGroup) return;
    const group = manager.getGroup(activeGroup?.id);
    const inputs = $state.snapshot(group?.inputs ?? {});
    delete inputs[key];
    activeGroup.inputs = inputs;
    manager.nodes = manager.nodes;
    manager.save();
  }

  const types = $derived(
    Array.from(
      new Set(
        manager?.registry
          ? manager.registry.getAllNodes()
            .flatMap(n =>
              Object.values(n.inputs ?? {})
                .map(v => v.type)
            )
          : []
      )
    )
  );

  let outputType = $derived(activeGroup?.outputs?.[0]?.type ?? 'unknown');

  $effect(() => {
    if (!activeGroup) return;
    const group = manager.getGroup(activeGroup?.id);
    const outputs = $state.snapshot(group?.outputs ?? []);
    if (outputs?.[0]?.type === outputType) return;
    activeGroup.outputs = [
      {
        label: outputs[0]?.label ?? 'Output',
        type: outputType
      }
    ];
    manager.nodes = manager.nodes;
    manager.save();
  });
</script>

{#if activeGroup}
  <div class='{node?"border-l-2 pl-3.5!":""} bg-layer-2 flex items-center h-[70px] border-b-1 border-l-selected border-b-outline pl-4'>
    <h3>Group Settings</h3>
  </div>
{/if}

{#if activeGroup}
  {#key activeGroup.id}
    <div class="p-4 group-settings">
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
        <SocketTable
          {types}
          onremove={handleRemoveInput}
          bind:inputs={activeGroup.inputs}
          colors={graphState?.colors?.getColors()}
        />
      </div>

      <label for="group-name mb-2">Group output</label>
      <div class="flex bg-layer-2 rounded-sm outline outline-outline w-min">
        <span
          style:background={graphState?.colors?.getColor(outputType)}
          class="block opacity-50 min-w-2 ml-2 w-2 h-2 my-auto rounded-sm"
        ></span>
        <select
          class="text-[0.9em] shrink-0 px-2 py-1 border-outline"
          bind:value={outputType}
        >
          {#each types as type (type)}
            <option>
              <span
                style="background: {graphState?.colors?.getColor(type)}; width: 5px; height: 5px; border-radius: 5px;"
              ></span>
              {type}
            </option>
          {/each}
        </select>
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
  }

  label {
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
