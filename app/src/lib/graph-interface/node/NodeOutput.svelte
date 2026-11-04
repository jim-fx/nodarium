<script lang="ts">
  import { getOutputLabel, type NodeInstance, type Socket } from '@nodarium/types';
  import { tryGetGraphManager, tryGetGraphState } from '../graph-state.svelte';
  import { createNodePath } from '../helpers/index.ts';

  type Props = {
    node: NodeInstance;
    index: number;
    isLast?: boolean;
  };

  const graph = tryGetGraphManager();
  const graphState = tryGetGraphState();

  const { node, index, isLast }: Props = $props();

  const nodeType = $derived(graph.getNodeType(node));
  const label = $derived(getOutputLabel(nodeType?.outputs?.[index]));
  const socketId = $derived(`${node.id}-${index}`);

  function handleMouseDown(ev: MouseEvent) {
    ev.preventDefault();
    ev.stopPropagation();
    graphState.setDownSocket({
      node,
      index,
      position: graphState.getSocketPosition(node, index)
    });
  }

  const cornerBottom = $derived(isLast ? 5 : 0);
  const aspectRatio = 0.5;

  const path = $derived(
    createNodePath({ depth: 6, height: 40, y: 50.5, cornerBottom, rightBump: true, aspectRatio })
  );
  const pathHover = $derived(
    createNodePath({ depth: 7, height: 44, y: 50.5, cornerBottom, rightBump: true, aspectRatio })
  );

  function getSocketType(s: Socket | null) {
    if (!s) return 'unknown';
    return graph.getSocketType(s.node, s.index) || 'unknown';
  }

  const hoverColor = $derived(graphState.colors.getColor(getSocketType(graphState.activeSocket)));
</script>

<div
  class="wrapper"
  data-node-type={node.type}
  style:--socket-color={hoverColor}
  class:possible-socket={graphState?.possibleSocketIds.has(socketId)}
>
  <div class="content">{label}</div>

  <div
    data-node-socket
    class="target"
    onmousedown={handleMouseDown}
    role="button"
    tabindex="0"
  >
  </div>

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
    height: 50px;
    transform: translateY(-0.5px);
  }

  .content {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    height: 100%;
    padding-inline: 20px;
    box-sizing: border-box;
  }

  .target {
    width: 30px;
    height: 30px;
    position: absolute;
    border-radius: 50%;
    top: 50%;
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

  .target:hover ~ svg path,
  .possible-socket svg path {
    d: var(--hover-path);
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
</style>
