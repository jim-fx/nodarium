<script lang="ts">
  import type { Edge, NodeInstance } from '@nodarium/types';
  import { Canvas } from '@threlte/core';
  import { HTML } from '@threlte/extras';
  import { createKeyMap } from '../../helpers/createKeyMap';
  import Background from '../background/Background.svelte';
  import AddMenu from '../components/AddMenu.svelte';
  import BoxSelection from '../components/BoxSelection.svelte';
  import Camera from '../components/Camera.svelte';
  import HelpView from '../components/HelpView.svelte';
  import Debug from '../debug/Debug.svelte';
  import EdgeEl from '../edges/Edge.svelte';
  import { getGraphManager, getGraphState } from '../graph-state.svelte';
  import NodeEl from '../node/Node.svelte';
  import { maxZoom, minZoom } from './constants';
  import { FileDropEventManager } from './drop.events';
  import { MouseEventManager } from './mouse.events';

  const {
    keymap,
    safePadding
  }: {
    keymap: ReturnType<typeof createKeyMap>;
    safePadding?: { left?: number; right?: number; bottom?: number; top?: number };
  } = $props();

  const graph = getGraphManager();
  const graphState = getGraphState();
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
    const newNode = graph.createNode({
      type: node.type,
      position: node.position,
      props: node.props
    });
    if (!newNode) return;

    if (graphState.activeSocket) {
      if (typeof graphState.activeSocket.index === 'number') {
        const socketType = graphState.activeSocket.node.state?.type?.outputs?.[
          graphState.activeSocket.index
        ];

        const input = Object.entries(newNode?.state?.type?.inputs || {}).find(
          (inp) => inp[1].type === socketType
        );

        if (input) {
          graph.createEdge(
            graphState.activeSocket.node,
            graphState.activeSocket.index,
            newNode,
            input[0]
          );
        }
      } else {
        const socketType = graphState.activeSocket.node.state?.type?.inputs?.[
          graphState.activeSocket.index
        ];

        const output = newNode.state?.type?.outputs?.find((out) => {
          if (socketType?.type === out) return true;
          if ((socketType?.accepts as string[])?.includes(out)) return true;
          return false;
        });

        if (output) {
          graph.createEdge(
            newNode,
            output.indexOf(output),
            graphState.activeSocket.node,
            graphState.activeSocket.index
          );
        }
      }
    }

    graphState.activeSocket = null;
    graphState.addMenuPosition = null;
  }

  function getSocketType(node: NodeInstance, index: number | string): string {
    const nodeType = graph.getNodeType(node);
    console.log({ nodeType, index });
    if (typeof index === 'string') {
      return nodeType?.inputs?.[index].type || 'unknown';
    }
    return nodeType?.outputs?.[index] || 'unknown';
  }
</script>

<svelte:window
  onmousemove={(ev) => mouseEvents.handleWindowMouseMove(ev)}
  onmouseup={(ev) => mouseEvents.handleWindowMouseUp(ev)}
/>

<div
  onwheel={(ev) => mouseEvents.handleMouseScroll(ev)}
  bind:this={graphState.wrapper}
  class="graph-wrapper"
  style="height: 100%"
  class:is-panning={graphState.isPanning}
  class:is-hovering={graphState.hoveredNodeId !== -1}
  aria-label="Graph"
  role="button"
  tabindex="0"
  bind:clientWidth={graphState.width}
  bind:clientHeight={graphState.height}
  onkeydown={(ev) => keymap.handleKeyboardEvent(ev)}
  onmousedown={(ev) => mouseEvents.handleMouseDown(ev)}
  oncontextmenu={(ev) => mouseEvents.handleContextMenu(ev)}
  {...fileDropEvents.getEventListenerProps()}
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

  <Canvas shadows={false} renderMode="on-demand" colorManagementEnabled={false}>
    <Camera
      bind:camera={graphState.camera}
      position={graphState.cameraPosition}
    />

    {#if graphState.backgroundType !== 'none'}
      <Background
        type={graphState.backgroundType}
        cameraPosition={graphState.cameraPosition}
        {maxZoom}
        {minZoom}
        width={graphState.width}
        height={graphState.height}
      />
    {/if}

    {#if graphState.boxSelection && graphState.mouseDown}
      <BoxSelection
        cameraPosition={graphState.cameraPosition}
        p1={{
          x: graphState.cameraPosition[0]
            + (graphState.mouseDown[0] - graphState.width / 2)
              / graphState.cameraPosition[2],
          y: graphState.cameraPosition[1]
            + (graphState.mouseDown[1] - graphState.height / 2)
              / graphState.cameraPosition[2]
        }}
        p2={{ x: graphState.mousePosition[0], y: graphState.mousePosition[1] }}
      />
    {/if}

    {#if graph.status === 'idle'}
      {#if graph.isInsideGroup}
        <HTML transform={false}>
          <button class="exit-group" onclick={() => graphState.exitGroupNode()}>
            ↑ Exit Group
          </button>
        </HTML>
      {/if}

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
          z={graphState.cameraPosition[2]}
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
          z={graphState.cameraPosition[2]}
          inputType={getSocketType(edge[0], edge[1])}
          outputType={getSocketType(edge[2], edge[3])}
          {x1}
          {y1}
          {x2}
          {y2}
        />
      {/each}

      <Debug />

      <HTML transform={false}>
        <div
          role="tree"
          id="graph"
          tabindex="0"
          class="wrapper"
          style:transform={`scale(${graphState.cameraPosition[2] * 0.1})`}
          class:hovering-sockets={graphState.activeSocket}
        >
          {#each graph.nodeArray as node, index (node.id)}
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

{#if graphState.showHelp}
  <HelpView registry={graph.registry} />
{/if}

<style>
  .graph-wrapper {
    position: relative;
    z-index: 0;
    transition: opacity 0.3s ease;
    height: 100%;
  }

  :global(.exit-group) {
    position: fixed;
    top: 12px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 1000;
    padding: 4px 12px;
    background: var(--color-layer-2);
    border: 1px solid var(--stroke);
    border-radius: 4px;
    color: inherit;
    font-size: 0.85em;
    cursor: pointer;
    opacity: 0.85;
  }

  :global(.exit-group:hover) {
    opacity: 1;
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
