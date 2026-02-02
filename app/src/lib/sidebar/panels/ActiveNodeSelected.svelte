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
    node: NodeInstance;
  };

  const { manager, node = $bindable() }: Props = $props();

  function filterInputs(inputs?: Record<string, NodeInput>) {
    const _inputs = $state.snapshot(
      inputs as Record<string, InternalNodeInput>
    );
    return Object.fromEntries(
      Object.entries(structuredClone(_inputs ?? {}))
        .filter(([, value]) => {
          return value.hidden === true;
        })
        .map(([key, value]) => {
          value.__node_type = node.state.type?.id;
          value.__node_input = key;
          return [key, value];
        })
    );
  }
  const nodeDefinition = filterInputs(node.state.type?.inputs);

  type Store = Record<string, number | number[]>;
  let store = $state<Store>(createStore(node?.props, nodeDefinition));
  function createStore(
    props: NodeInstance['props'],
    inputs: Record<string, NodeInput>
  ): Store {
    const store: Store = {};
    Object.keys(inputs).forEach((key) => {
      if (props) {
        const value = props[key] || inputs[key].value;
        if (Array.isArray(value) || typeof value === 'number') {
          store[key] = value;
        } else {
          console.error('Wrong error');
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

  $effect(() => {
    if (store) {
      updateNode();
    }
  });
</script>

{#if Object.keys(nodeDefinition).length}
  <NestedSettings
    id="activeNodeSettings"
    bind:value={store}
    type={nodeDefinition}
  />
{:else}
  <p class="mx-4">Node has no settings</p>
{/if}
