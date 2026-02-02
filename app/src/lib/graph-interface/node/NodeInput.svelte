<script lang="ts">
  import type { NodeInput, NodeInstance } from '@nodarium/types';
  import { Input } from '@nodarium/ui';
  import type { GraphManager } from '../graph-manager.svelte';

  type Props = {
    node: NodeInstance;
    input: NodeInput;
    id: string;
    elementId?: string;
    graph?: GraphManager;
  };

  const {
    node = $bindable(),
    input,
    id,
    elementId = `input-${Math.random().toString(36).substring(7)}`,
    graph
  }: Props = $props();

  function getDefaultValue() {
    if (node?.props?.[id] !== undefined) return node?.props?.[id] as number;
    if ('value' in input && input?.value !== undefined) {
      return input?.value as number;
    }
    if (input.type === 'boolean') return 0;
    if (input.type === 'float') return 0.5;
    if (input.type === 'integer') return 0;
    if (input.type === 'select') return 0;
    return 0;
  }

  let value = $state(getDefaultValue());

  $effect(() => {
    if (value !== undefined && node?.props?.[id] !== value) {
      node.props = { ...node.props, [id]: value };
      if (graph) {
        graph.save();
        graph.execute();
      }
    }
  });
</script>

<Input id="input-{elementId}" {input} bind:value />
