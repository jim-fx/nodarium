<script lang="ts">
  import type { NodeInput, NodeInstance, Socket } from '@nodarium/types';
  import { getGraphManager, getGraphState } from '../graph-state.svelte';
  import { createNodePath } from '../helpers';
  import { getParameterHeight, getSocketPosition } from '../helpers/nodeHelpers';
  import NodeInputEl from './NodeInput.svelte';

  type Props = {
    node: NodeInstance;
    input: NodeInput;
    id: string;
    isLast?: boolean;
  };

  const graph = getGraphManager();
  const graphState = getGraphState();
  const graphId = graph?.id;
  const elementId = `input-${Math.random().toString(36).substring(7)}`;

  let { node = $bindable(), input, id, isLast }: Props = $props();

  const nodeType = $derived(node.state.type!);

  const inputType = $derived(nodeType.inputs?.[id]);

  const socketId = $derived(`${node.id}-${id}`);
  const height = $derived(getParameterHeight(nodeType, id));

  function handleMouseDown(ev: MouseEvent) {
    ev.preventDefault();
    ev.stopPropagation();
    graphState.setDownSocket({
      node,
      index: id,
      position: getSocketPosition(node, id)
    });
  }

  const leftBump = $derived(nodeType.inputs?.[id].internal !== true);
  const cornerBottom = $derived(isLast ? 5 : 0);
  const aspectRatio = 0.5;

  const path = $derived(
    createNodePath({
      depth: 6,
      height: 2000 / height,
      y: 50.5,
      cornerBottom,
      leftBump,
      aspectRatio
    })
  );
  const pathHover = $derived(
    createNodePath({
      depth: 7,
      height: 2200 / height,
      y: 50.5,
      cornerBottom,
      leftBump,
      aspectRatio
    })
  );

  function getSocketType(s: Socket | null) {
    if (!s) return 'unknown';
    if (typeof s.index === 'string') {
      return s.node.state.type?.inputs?.[s.index].type || 'unknown';
    }
    return s.node.state.type?.outputs?.[s.index] || 'unknown';
  }

  const socketType = $derived(getSocketType(graphState.activeSocket));
  const hoverColor = $derived(graphState.colors.getColor(socketType));
</script>

<div
  class="wrapper"
  data-node-type={node.type}
  data-node-input={id}
  style:height="{height}px"
  style:--socket-color={hoverColor}
  class:possible-socket={graphState?.possibleSocketIds.has(socketId)}
>
  {#key id && graphId}
    <div class="content" class:disabled={graph?.inputSockets?.has(socketId)}>
      {#if inputType?.label !== ''}
        <label for={elementId} title={input.description}>{input.label || id}</label>
      {/if}
      {#if inputType?.external !== true}
        <NodeInputEl {graph} {elementId} bind:node {input} {id} />
      {/if}
    </div>

    {#if node?.state?.type?.inputs?.[id]?.internal !== true}
      <div
        data-node-socket
        class="target"
        onmousedown={handleMouseDown}
        role="button"
        tabindex="0"
      >
      </div>
    {/if}
  {/key}

  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox="0 0 100 100"
    preserveAspectRatio="none"
    style:--path={`path("${path}")`}
    style:--hover-path={`path("${pathHover}")`}
  >
    <path vector-effect="non-scaling-stroke"></path>
  </svg>
</div>

<style>
  .wrapper {
    position: relative;
    width: 100%;
    transform: translateY(-0.5px);
  }

  .target {
    width: 30px;
    height: 30px;
    position: absolute;
    border-radius: 50%;
    top: 50%;
    transform: translateY(-50%) translateX(-50%);
  }

  .possible-socket .target::before {
    content: "";
    position: absolute;
    width: 30px;
    height: 30px;
    border-radius: 100%;
    box-shadow: 0px 0px 10px var(--socket-color);
    background-color: var(--socket-color);
    outline: solid thin var(--socket-color);
    opacity: 0.5;
    z-index: -10;
  }

  .target:hover ~ svg path{
    d: var(--hover-path);
  }

  .content {
    position: relative;
    display: flex;
    flex-direction: column;
    padding-inline: 20px;
    height: 100%;
    justify-content: center;
    gap: 10px;
    box-sizing: border-box;
  }

  svg {
    position: absolute;
    box-sizing: border-box;
    width: calc(100% - 2px);
    height: 100%;
    overflow: visible;
    top: 0;
    left: 1px;
    z-index: -1;
  }

  svg path {
    transition: d 0.3s ease, fill 0.3s ease;
    fill: var(--color-layer-1);
    stroke: var(--stroke);
    stroke-width: var(--stroke-width);
    d: var(--path);

    stroke-linejoin: round;
    shape-rendering: geometricPrecision;
  }

  .content.disabled {
    opacity: 0.2;
  }

  .possible-socket svg path {
     d: var(--hover-path);
  }
</style>
