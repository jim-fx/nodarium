<script lang="ts">
  import type { GraphManager } from '#lib/graph-interface/graph-manager.svelte.ts';
  import NestedSettings from '#lib/settings/NestedSettings.svelte';
  import type { NodeId, NodeInput, NodeInstance } from '@nodarium/types';

  type InternalNodeInput = NodeInput & {
    __node_type?: NodeId;
    __node_input: string;
  };

  type Props = {
    manager: GraphManager;
    node: NodeInstance | undefined;
  };

  const { manager, node }: Props = $props();

  function filterInputs(inputs?: Record<string, NodeInput>) {
    if (!node) return {};
    return Object.fromEntries(
      Object.entries(inputs ?? {})
        // exposed inputs are edited on the node itself
        .filter(([key, value]) => {
          return value.hidden === true && !node.exposed?.includes(key);
        })
        .map(([key, value]) => {
          const v = value as InternalNodeInput;
          v.__node_type = node.state.type?.id;
          v.__node_input = key;
          return [key, v];
        })
    );
  }
  // The parent re-creates this panel for every node, so initial values are enough
  // svelte-ignore state_referenced_locally
  const nodeDefinition = node ? filterInputs(node.state.type?.inputs) : {};

  type Store = Record<string, number | number[] | string>;
  // svelte-ignore state_referenced_locally
  let store = $state<Store>(createStore(node?.props, nodeDefinition));
  function createStore(
    props: NodeInstance['props'],
    inputs: Record<string, NodeInput>
  ): Store {
    const store: Store = {};
    Object.keys(inputs).forEach((key) => {
      if (props) {
        const value = props[key] !== undefined ? props[key] : inputs[key].value;
        // older projects stored the name of the chosen select option
        if (Array.isArray(value) || typeof value === 'number' || typeof value === 'string') {
          store[key] = value;
        } else if (typeof value === 'boolean') {
          store[key] = value ? 1 : 0;
        } else {
          console.error('Wrong error', { value });
        }
      }
    });
    return store;
  }

  let lastPropsHash = '';
  function updateNode() {
    if (!node || !store) return;
    let needsUpdate = false;
    Object.keys(store).forEach((_key: string) => {
      node.props = node.props || {};
      const key = _key as keyof typeof store;
      if (node && store) {
        needsUpdate = true;
        const value = store[key];
        if (typeof value === 'string') {
          // node props store the index of the chosen select option
          const input = nodeDefinition[key];
          const options: unknown[] = input && 'options' in input ? input.options ?? [] : [];
          node.props[key] = Math.max(0, options.indexOf(value));
        } else if (value !== undefined) {
          node.props[key] = value;
        }
      }
    });

    let propsHash = JSON.stringify(node.props);
    if (propsHash === lastPropsHash) {
      return;
    }
    lastPropsHash = propsHash;

    if (needsUpdate) {
      manager.save();
      manager.execute();
    }
  }

  const isGroupInstance = $derived(node?.type === '__internal/group/instance');

  $effect(() => {
    if (store) {
      updateNode();
    }
  });
</script>

{#if !isGroupInstance && Object.keys(nodeDefinition).length}
  <div class='{node?"border-l-2 pl-3.5!":""} bg-layer-2 flex items-center h-[70px] border-b-1 border-l-selected border-b-outline pl-4'>
    <h3>Node Settings</h3>
  </div>
  <NestedSettings
    id="activeNodeSettings"
    bind:value={store}
    type={nodeDefinition}
    inputAction={pinButton}
  />
{/if}

{#snippet pinButton(key: string)}
  <button
    type="button"
    aria-label="Show on node"
    class="flex cursor-pointer items-center border-none bg-transparent p-0 opacity-50 hover:opacity-100"
    onclick={() => node && manager.setInputExposed(node, key, true)}
  >
    <span class="block h-4 w-4 i-[tabler--pin]"></span>
  </button>
{/snippet}
