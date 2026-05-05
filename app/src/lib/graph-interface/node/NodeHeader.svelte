<script lang="ts">
  import { appSettings } from '$lib/settings/app-settings.svelte';
  import type { NodeInstance, Socket } from '@nodarium/types';
  import { getGraphManager, getGraphState } from '../graph-state.svelte';
  import { createNodePath } from '../helpers/index.js';

  const graphState = getGraphState();
  const graph = getGraphManager();

  const { node }: { node: NodeInstance } = $props();

  function handleMouseDown(event: MouseEvent) {
    event.stopPropagation();
    event.preventDefault();
    if ('state' in node) {
      graphState.setDownSocket?.({
        node,
        index: 0,
        position: graphState.getSocketPosition?.(node, 0)
      });
    }
  }

  const cornerTop = 10;
  const nodeType = $derived(graph.getNodeType(node));
  const rightBump = $derived(
    !!nodeType?.outputs?.length && node.type !== '__internal/group/input'
  );
  const cornerBottom = $derived(
    node.type === '__internal/group/input'
      ? (Object.keys(nodeType?.inputs ?? {}).length ? 0 : 10)
      : node.type === '__internal/group/output'
      ? (nodeType?.outputs?.length ? 0 : 10)
      : 0
  );

  const aspectRatio = 0.25;

  const path = $derived(
    createNodePath({
      depth: 5.5,
      height: 34,
      y: 49,
      cornerTop,
      cornerBottom,
      rightBump,
      aspectRatio
    })
  );
  const pathHover = $derived(
    createNodePath({
      depth: 7,
      height: 40,
      y: 49,
      cornerTop,
      cornerBottom,
      rightBump,
      aspectRatio
    })
  );

  const socketId = $derived(`${node.id}-${0}`);

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
  data-node-id={node.id}
  data-node-type={node.type}
  style:--socket-color={hoverColor}
  class:possible-socket={graphState?.possibleSocketIds.has(socketId)}
>
  <div class="content">
    {#if appSettings.value.debug.advancedMode}
      <span class="bg-white text-black! mr-2 px-1 rounded-sm opacity-30">{node.id}</span>
    {/if}
    {nodeType?.meta?.title || node.type?.split('/').pop()}
  </div>
  {#if rightBump}
    <div
      class="target"
      role="button"
      tabindex="0"
      onmousedown={handleMouseDown}
    >
    </div>
  {/if}
  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox="0 0 100 100"
    width="100"
    height="100"
    preserveAspectRatio="none"
    style={`
      --path: path("${path}");
      --hover-path: path("${pathHover}");
    `}
  >
    <path vector-effect="non-scaling-stroke" stroke="white" stroke-width="0.1"></path>
  </svg>
</div>

<style>
  .wrapper {
    position: relative;
    width: 100%;
    height: 50px;
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
    opacity: 0.7;
    z-index: -10;
  }

  .target {
    position: absolute;
    right: 0px;
    top: 50%;
    transform: translateX(50%) translateY(-50%);
    height: 30px;
    width: 30px;
    z-index: 100;
    border-radius: 50%;
  }

  .target:hover + svg path {
    d: var(--hover-path);
  }

  svg {
    position: absolute;
    top: 1px;
    left: 1px;
    z-index: -1;
    box-sizing: border-box;
    width: calc(100% - 2px);
    height: calc(100% - 1px);
    overflow: visible;
  }

  svg path {
    stroke-width: 0.2px;
    transition:
      d 0.3s ease,
      fill 0.3s ease;
    fill: var(--color-layer-2);
    stroke: var(--stroke);
    stroke-width: var(--stroke-width);
    d: var(--path);

    stroke-linejoin: round;
    shape-rendering: geometricPrecision;
  }

  .content {
    font-size: 1em;
    display: flex;
    align-items: center;
    padding-left: 20px;
    height: 100%;
  }

  svg:hover path {
    d: var(--hover-path) !important;
  }
</style>
