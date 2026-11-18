<script lang="ts">
  import { GROUP_INPUT } from '@nodarium/types';
  import type { NodeInput, NodeInstance, Socket } from '@nodarium/types';
  import { describeAngles } from '@nodarium/ui';
  import { tryGetGraphManager, tryGetGraphState } from '../graph-state.svelte';
  import { createNodePath } from '../helpers/index.ts';
  import { getParameterHeight } from '../helpers/nodeHelpers';
  import { connectionDescription, socketName } from '../helpers/socketNames';
  import NodeInputEl from './NodeInput.svelte';

  type Props = {
    node: NodeInstance;
    input: NodeInput;
    id: string;
    isLast?: boolean;
  };

  const graph = tryGetGraphManager();
  const graphState = tryGetGraphState();
  const graphId = graph?.id;
  const elementId = `input-${Math.random().toString(36).substring(7)}`;

  let { node = $bindable(), input, id, isLast }: Props = $props();

  let nodeType = $derived(graph.getNodeType(node)!);

  const inputType = $derived(nodeType.inputs?.[id]);

  // a hidden input that is shown on this node, see GraphManager.setInputExposed()
  const isExposed = $derived(node.state?.type?.inputs?.[id]?.hidden === true);

  const height = $derived(getParameterHeight(nodeType, id));

  // the rows of the Group Input node are its outputs
  const socketIndex = $derived(
    node.type === GROUP_INPUT ? Object.keys(nodeType?.inputs ?? {}).indexOf(id) : id
  );
  const socketId = $derived(`${node.id}-${socketIndex}`);
  const name = $derived(socketName(node, nodeType, socketIndex));
  const socket = () => ({
    node,
    index: socketIndex,
    position: graphState.getSocketPosition(node, socketIndex)
  });

  function handleMouseDown(ev: MouseEvent) {
    ev.preventDefault();
    ev.stopPropagation();
    graphState.pressSocket(socket(), ev);
  }

  function handleKeyDown(ev: KeyboardEvent) {
    if (ev.key !== 'Enter' && ev.key !== ' ') return;
    ev.preventDefault();
    graphState.activateSocket(socket());
  }

  const leftBump = $derived(
    nodeType.inputs?.[id].internal !== true && node.type !== GROUP_INPUT
  );
  const rightBump = $derived(node.type === GROUP_INPUT);
  const cornerBottom = $derived(isLast ? 5 : 0);
  const aspectRatio = 0.5;

  const path = $derived(
    createNodePath({
      depth: 6,
      height: 2000 / height,
      y: 50.5,
      cornerBottom,
      rightBump,
      leftBump,
      aspectRatio
    })
  );
  const pathHover = $derived(
    createNodePath({
      depth: 7,
      height: 2200 / height,
      y: 50.5,
      rightBump,
      cornerBottom,
      leftBump,
      aspectRatio
    })
  );

  function getSocketType(s: Socket | null) {
    if (!s) return 'unknown';
    return graph.getSocketType(s.node, s.index) || 'unknown';
  }

  const socketType = $derived(getSocketType(graphState.connectingSocket));
  const hoverColor = $derived(graphState.colors.getColor(socketType));
  const edgeIn = $derived(graph?.inputSockets?.get(socketId));
  const disabled = $derived(!!edgeIn);
  const connection = $derived(
    edgeIn && connectionDescription(edgeIn[0], graph.getNodeType(edgeIn[0]), edgeIn[1])
  );
</script>

<div
  class="wrapper"
  data-node-type={node.type}
  class:is-group-input={node.type === GROUP_INPUT}
  data-node-input={id}
  style:height="{height}px"
  style:--socket-color={hoverColor}
  class:possible-socket={graphState?.possibleSocketIds.has(socketId)}
>
  {#key id && graphId}
    <div class="content" class:disabled={disabled}>
      {#if isExposed}
        <div class="flex items-center justify-between gap-2">
          {#if inputType?.label !== ''}
            <label for={elementId} title={input.description && describeAngles(input.description)}>{
              input.label || id
            }</label>
          {/if}
          <button
            type="button"
            aria-label="Hide from node"
            class="flex cursor-pointer items-center border-none bg-transparent p-0 opacity-50 hover:opacity-100"
            onclick={() => graph.setInputExposed(node, id, false)}
          >
            <span class="block h-3.5 w-3.5 i-[tabler--pin-filled]"></span>
          </button>
        </div>
      {:else if inputType?.label !== ''}
        <label for={elementId} title={input.description && describeAngles(input.description)}>{
          input.label || id
        }</label>
      {/if}
      {#if inputType?.external !== true}
        <NodeInputEl {graph} {elementId} bind:node {input} {id} />
      {/if}
    </div>

    {#if connection}
      <span id="{elementId}-connection" class="sr-only">{connection}</span>
    {/if}

    {#if inputType?.internal !== true}
      <div
        data-node-socket
        class="target"
        onmousedown={handleMouseDown}
        onkeydown={handleKeyDown}
        role="button"
        tabindex="0"
        aria-label={name}
        aria-describedby={connection ? `${elementId}-connection` : undefined}
        aria-pressed={graphState?.isPicked(node, socketIndex)}
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

  .is-group-input .target {
    right: 0px;
    transform: translateY(-50%) translateX(50%);
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
