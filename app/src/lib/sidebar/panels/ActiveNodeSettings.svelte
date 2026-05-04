<script lang="ts">
  import type { GraphManager } from '$lib/graph-interface/graph-manager.svelte';
  import NestedSettings from '$lib/settings/NestedSettings.svelte';
  import type { NodeId, NodeInput, NodeInstance } from '@nodarium/types';

  type InternalNodeInput = NodeInput & {
    __node_type?: NodeId;
    __node_input: string;
  };

  type Props = {
    manager: GraphManager;
    node: NodeInstance | undefined;
  };

  const { manager, node = $bindable() }: Props = $props();

  function filterInputs(inputs?: Record<string, NodeInput>) {
    if (!node) return {};
    return Object.fromEntries(
      Object.entries(inputs ?? {})
        .filter(([, value]) => {
          return value.hidden === true;
        })
        .map(([key, value]) => {
          const v = value as InternalNodeInput;
          v.__node_type = node.state.type?.id;
          v.__node_input = key;
          return [key, v];
        })
    );
  }
  const nodeDefinition = node ? filterInputs(node.state.type?.inputs) : {};

  type Store = Record<string, number | number[]>;
  let store = $state<Store>(createStore(node?.props, nodeDefinition));
  function createStore(
    props: NodeInstance['props'],
    inputs: Record<string, NodeInput>
  ): Store {
    const store: Store = {};
    Object.keys(inputs).forEach((key) => {
      if (props) {
        const value = props[key] !== undefined ? props[key] : inputs[key].value;
        if (Array.isArray(value) || typeof value === 'number') {
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
        if (value !== undefined) {
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
  />
{/if}
