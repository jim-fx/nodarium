<script lang="ts">
  import NodeHTML from '$lib/graph-interface/node/NodeHTML.svelte';
  import Grid from '$lib/grid';
  import { localState } from '$lib/helpers/localState.svelte';
  import { IndexDBCache, RemoteNodeRegistry } from '$lib/node-registry/index';
  import Panel from '$lib/sidebar/Panel.svelte';
  import Sidebar from '$lib/sidebar/Sidebar.svelte';
  import { type NodeId, type NodeInstance } from '@nodarium/types';
  import { concatEncodedArrays, createWasmWrapper, encodeNestedArray } from '@nodarium/utils';

  const registryCache = new IndexDBCache('node-registry');
  const nodeRegistry = new RemoteNodeRegistry('', registryCache);

  let activeNode = localState<NodeId | undefined>(
    'node.dev.activeNode',
    undefined
  );

  let nodeWasm = $state<ArrayBuffer>();
  let nodeInstance = $state<NodeInstance>();
  let nodeWasmWrapper = $state<ReturnType<typeof createWasmWrapper>>();

  async function fetchNodeData(nodeId?: NodeId) {
    nodeWasm = undefined;
    nodeInstance = undefined;

    if (!nodeId) return;

    const data = await nodeRegistry.fetchNodeDefinition(nodeId);
    nodeWasm = await nodeRegistry.fetchArrayBuffer('nodes/' + nodeId + '.wasm');
    nodeInstance = {
      id: 0,
      type: nodeId,
      position: [0, 0] as [number, number],
      props: {},
      state: {
        type: data
      }
    };
    try {
      nodeWasmWrapper = createWasmWrapper(nodeWasm);
    } catch (e) {
      console.error(`Failed to create node wrapper for ${nodeId}`, e);
    }
  }

  let graphSettings = $state<Record<string, any>>({});
  let graphSettingTypes = $state({
    randomSeed: { type: "boolean", value: false },
  });

  $effect(() => {
    if (nodeInstance?.props && nodeWasmWrapper) {
      const keys = Object.keys(nodeInstance.state.type?.inputs || {});
      let ins = Object.values(nodeInstance.props) as (number[] | number)[];
      if (keys[0] === 'plant') {
        ins = [[0, 0, 0, 0, 0, 0, 0, 0], ...ins];
      }
      const inputs = concatEncodedArrays(encodeNestedArray(ins));
      nodeWasmWrapper?.execute(inputs);
    }
  });
</script>

<Grid.Row>
  <Grid.Cell>
    {#if result}
      <pre><code>{JSON.stringify(decodeNestedArray(result))}</code></pre>
    {/if}
  </Grid.Cell>

  <Grid.Cell>
    <div class="h-screen w-[80vw] overflow-y-auto"></div>
  </Grid.Cell>
</Grid.Row>

<Sidebar>
  <Panel
    id="node-store"
    classes="text-green-400"
    title="Node Store"
    icon="i-[tabler--database]"
  >
    <div class="p-4 flex flex-col gap-2">
      {#await nodeRegistry.fetchCollection('max/plantarium')}
        <p>Loading Nodes...</p>
      {:then result}
        {#each result.nodes as n (n.id)}
          <button
            class="
              cursor-pointer p-2 bg-layer-1 {activeNode.value === n.id
              ? 'outline outline-offset-1'
              : ''}
            "
            onclick={() => (activeNode.value = n.id)}
          >
            {n.id}
          </button>
        {/each}
      {/await}
    </div>
  </Panel>
</Sidebar>

<style>
  :global body {
    height: 100vh;
    width: 100vw;
    overflow: hidden;
  }
</style>
