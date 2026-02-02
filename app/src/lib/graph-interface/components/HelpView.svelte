<script lang="ts">
  import type { NodeDefinition, NodeRegistry } from '@nodarium/types';
  import { onMount } from 'svelte';

  let mx = $state(0);
  let my = $state(0);

  let node: NodeDefinition | undefined = $state(undefined);
  let input: string | undefined = $state(undefined);

  let wrapper: HTMLDivElement;
  type Props = { registry: NodeRegistry };
  const { registry }: Props = $props();

  let width = $state(0);

  function handleMouseOver(ev: MouseEvent) {
    let target = ev.target as HTMLElement | null;
    mx = ev.clientX;
    my = ev.clientY;
    if (!target) return;

    const closest = target?.closest?.('[data-node-type]');

    if (!closest) {
      node = undefined;
      return;
    }

    let nodeType = closest.getAttribute('data-node-type');
    let nodeInput = closest.getAttribute('data-node-input');

    if (!nodeType) {
      node = undefined;
      return;
    }
    node = registry.getNode(nodeType);
    input = nodeInput || undefined;
  }

  onMount(() => {
    const style = wrapper.parentElement?.style;
    style?.setProperty('cursor', 'help');
    return () => {
      style?.removeProperty('cursor');
    };
  });
</script>

<svelte:window onmousemove={handleMouseOver} />

<div
  class="help-wrapper p-4"
  class:visible={node}
  bind:clientWidth={width}
  style="--my: {my}px; --mx: {Math.min(mx, window.innerWidth - width - 20)}px"
  bind:this={wrapper}
>
  <p class="m-0 text-light opacity-40 flex items-center gap-3 mb-4">
    <span class="i-tabler-help block w-4 h-4"></span>
    {node?.id.split('/').at(-1) || 'Help'}
    {#if input}
      <span>> {input}</span>
    {/if}
  </p>
  {#if node}
    <div class="mb-4">
      {#if input}
        {node?.inputs?.[input]?.description || input}
      {:else if node?.meta?.description}
        {node?.meta?.description}
      {:else}
        <div class="text-xs opacity-30 mb-4">{node.id}</div>
      {/if}
    </div>

    {#if !input}
      <div>
        <span class="i-tabler-arrow-right opacity-30">-></span>
        {node?.outputs?.map((o) => o).join(', ') ?? 'nothing'}
      </div>
    {/if}
  {/if}
</div>

<style>
  .help-wrapper {
    position: fixed;
    pointer-events: none;
    transform: translate(var(--mx), var(--my));
    background: var(--layer-1);
    border-radius: 5px;
    top: 10px;
    left: 10px;
    max-width: 250px;
    border: 1px solid var(--outline);
    z-index: 10000;
    display: none;
  }

  .visible {
    display: block;
  }
</style>
