<script lang="ts">
  import type { NodeInput, NodeInstance } from '@nodarium/runtime';
  import { Input } from '@nodarium/ui';
  import { untrack } from 'svelte';
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

  /** What the node runs with: its own value, or the default of the input */
  function nodeValue() {
    const value = node?.props?.[id] ?? ('value' in input ? input.value : undefined);
    return $state.snapshot(value ?? (input.type === 'float' ? 0.5 : 0));
  }

  function isSame(a: unknown, b: unknown) {
    if (!Array.isArray(a) || !Array.isArray(b)) return a === b;
    return a.length === b.length && a.every((v, i) => v === b[i]);
  }

  // A copy: the inputs change arrays in place, and the default belongs to the definition
  let value = $state(nodeValue());

  // The value can also change from outside, by a drag in the viewer or by undo
  $effect(() => {
    const shown = nodeValue();
    untrack(() => {
      if (!isSame($state.snapshot(value), shown)) value = shown;
    });
  });

  // Only a changed value is written: a node that shows the defaults of its inputs keeps no props
  $effect(() => {
    const changed = $state.snapshot(value);
    untrack(() => {
      if (isSame(changed, nodeValue())) return;
      node.props = { ...node.props, [id]: changed } as NodeInstance['props'];
      graph?.commit({ coalesce: true });
    });
  });
</script>

<Input id="input-{elementId}" {input} bind:value />
