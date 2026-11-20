<script lang="ts">
  import type { Edge, NodeInstance } from '@nodarium/types';
  import type { createKeyMap } from '@nodarium/ui';
  import { Canvas } from '@threlte/core';
  import { HTML } from '@threlte/extras';
  import Background from '../background/Background.svelte';
  import AddMenu from '../components/AddMenu.svelte';
  import BoxSelection from '../components/BoxSelection.svelte';
  import Camera from '../components/Camera.svelte';
  import GroupBreadcrumps from '../components/GroupBreadcrumps.svelte';
  import HelpView from '../components/HelpView.svelte';
  import OverlayScale from '../components/OverlayScale.svelte';
  import Debug from '../debug/Debug.svelte';
  import EdgeEl from '../edges/Edge.svelte';
  import FrameEl from '../frame/Frame.svelte';
  import { getGraphManager, getGraphState } from '../graph-state.svelte';
  import NodeEl from '../node/Node.svelte';
  import SocketHighlights from '../node/SocketHighlights.svelte';
  import { maxZoom, minZoom, WEBGL_NODE_MAX_ZOOM } from './constants';
  import { FileDropEventManager } from './drop.events';
  import { MouseEventManager } from './mouse.events';
  import ZoomIndicator from './ZoomIndicator.svelte';

  const { keymap }: { keymap: ReturnType<typeof createKeyMap> } = $props();

  let overlayEl = $state<HTMLElement>();

  const graph = getGraphManager();
  const graphState = getGraphState();
  const safePadding = $derived(graphState.options.safePadding);
  const fileDropEvents = new FileDropEventManager(graph, graphState);
  const mouseEvents = new MouseEventManager(graph, graphState);

  function getEdgePosition(edge: Edge) {
    const fromNode = graph.nodes.get(edge[0].id);
    const toNode = graph.nodes.get(edge[2].id);

    // This check is important because nodes might not be there during some transitions.
    if (!fromNode || !toNode) {
      return [0, 0, 0, 0];
    }

    const pos1 = graphState.getSocketPosition(fromNode, edge[1]);
    const pos2 = graphState.getSocketPosition(toNode, edge[3]);
    return [pos1[0], pos1[1], pos2[0], pos2[1]];
  }

  function handleNodeCreation(node: NodeInstance) {
    if (!graph.createNodeFromSocket(node, graphState.activeSocket)) return;
    graphState.activeSocket = null;
    graphState.addMenuPosition = null;
  }

  function getSocketType(node: NodeInstance, index: number | string): string {
    return graph.getSocketType(node, index) || 'unknown';
  }
</script>

<svelte:window
  onmousedown={() => mouseEvents.handleWindowMouseDown()}
  onkeydown={(ev) => ev.key === 'Escape' && graphState.unpickSocket()}
  onmousemove={(ev) => mouseEvents.handleWindowMouseMove(ev)}
  onmouseup={(ev) => mouseEvents.handleWindowMouseUp(ev)}
/>

<div
  onwheel={(ev) => mouseEvents.handleMouseScroll(ev)}
  bind:this={graphState.wrapper}
  class="graph-wrapper"
  style="height: 100%"
  class:is-inside-group={graph.isInsideGroup}
  class:is-panning={graphState.isPanning}
  class:is-hovering={graphState.hoveredNodeId !== -1}
  aria-label="Graph"
  role="button"
  tabindex="0"
  bind:clientWidth={graphState.camera.width}
  bind:clientHeight={graphState.camera.height}
  style:--padding-right="{safePadding?.right || 0}px"
  onkeydown={(ev) => keymap.handleKeyboardEvent(ev)}
  onmousedown={(ev) => mouseEvents.handleMouseDown(ev)}
  oncontextmenu={(ev) => mouseEvents.handleContextMenu(ev)}
  {...graphState.options.valuesOnly ? {} : fileDropEvents.getEventListenerProps()}
>
  <input
    type="file"
    accept="application/wasm,application/json"
    id="drop-zone"
    disabled={!graphState.isDragging}
    ondragend={(ev) => fileDropEvents.handleDragEnd(ev)}
    ondragleave={(ev) => fileDropEvents.handleDragEnd(ev)}
  />
  <label for="drop-zone"></label>

  <GroupBreadcrumps />

  <Canvas shadows={false} renderMode="on-demand">
    <Camera position={graphState.camera.position} />

    {#if graphState.options.backgroundType !== 'none'}
      <Background
        type={graphState.options.backgroundType}
        cameraPosition={graphState.camera.position}
        {maxZoom}
        {minZoom}
        width={graphState.camera.width}
        height={graphState.camera.height}
      />
    {/if}

    {#if graphState.boxSelection && graphState.mouseDown}
      {@const [x, y] = graphState.camera.toWorld(graphState.mouseDown[0], graphState.mouseDown[1])}
      <BoxSelection
        cameraPosition={graphState.camera.position}
        p1={{ x, y }}
        p2={{ x: graphState.mousePosition[0], y: graphState.mousePosition[1] }}
      />
    {/if}

    {#if graph.status === 'idle'}
      {#if graphState.addMenuPosition}
        <AddMenu
          onnode={handleNodeCreation}
          paddingTop={safePadding?.top}
          paddingRight={safePadding?.right}
          paddingBottom={safePadding?.bottom}
          paddingLeft={safePadding?.left}
        />
      {/if}

      {#if graphState.activeSocket}
        <EdgeEl
          z={graphState.camera.position[2]}
          inputType={getSocketType(graphState.activeSocket.node, graphState.activeSocket.index)}
          outputType={getSocketType(graphState.activeSocket.node, graphState.activeSocket.index)}
          x1={graphState.activeSocket.position[0]}
          y1={graphState.activeSocket.position[1]}
          x2={graphState.edgeEndPosition?.[0] ?? graphState.mousePosition[0]}
          y2={graphState.edgeEndPosition?.[1] ?? graphState.mousePosition[1]}
        />
      {/if}

      {#each graph.edges as edge (edge)}
        {@const [x1, y1, x2, y2] = getEdgePosition(edge)}
        <EdgeEl
          id={graph.getEdgeId(edge)}
          z={graphState.camera.position[2]}
          inputType={getSocketType(edge[0], edge[1])}
          outputType={getSocketType(edge[2], edge[3])}
          {x1}
          {y1}
          {x2}
          {y2}
        />
      {/each}

      {#if graphState.possibleSockets.length && graphState.camera.position[2] < WEBGL_NODE_MAX_ZOOM}
        <SocketHighlights />
      {/if}

      <Debug />

      <OverlayScale el={overlayEl} />

      <HTML transform={false}>
        <div
          role="group"
          aria-label="Nodes"
          id="graph"
          class="wrapper"
          bind:this={overlayEl}
          class:hovering-sockets={graphState.activeSocket}
        >
          {#each graph.frames as frame (frame.id)}
            <FrameEl {frame} />
          {/each}
          {#each graph.nodeArray as node, index (node)}
            <NodeEl
              bind:node={graph.nodeArray[index]}
              inView={node ? graphState.isNodeInView(node) : false}
            />
          {/each}
        </div>
      </HTML>
    {:else if graph.status === 'loading'}
      <span>Loading</span>
    {:else if graph.status === 'error'}
      <span>Error</span>
    {/if}
  </Canvas>
</div>

{#if graphState.options.showHelp}
  <HelpView registry={graph.registry} />
{/if}

<ZoomIndicator {safePadding} />

<style>
  .graph-wrapper {
    position: relative;
    z-index: 0;
    transition: opacity 0.3s ease;
    height: 100%;
  }

  .wrapper {
    position: absolute;
    z-index: 100;
    width: 0px;
    height: 0px;
  }

  .is-hovering {
    cursor: pointer;
  }

  .is-panning {
    cursor: grab;
  }

  input {
    position: absolute;
    z-index: 1;
    width: 100%;
    height: 100%;
    background: var(--color-layer-2);
    opacity: 0;
  }
  input:disabled {
    opacity: 0;
    pointer-events: none;
  }
  input:disabled + label {
    opacity: 0;
    pointer-events: none;
  }

  label {
    position: absolute;
    z-index: 1;
    top: 10px;
    left: 10px;
    border-radius: 5px;
    width: calc(100% - 20px);
    height: calc(100% - 25px);
    border: dashed 4px var(--color-layer-2);
    background: var(--color-layer-1);
    opacity: 0.5;
  }
</style>
