<script lang="ts">
  import type { NodeId, NodeInstance } from '@nodarium/types';
  import { HTML } from '@threlte/extras';
  import { onMount } from 'svelte';
  import { getGraphManager, getGraphState } from '../graph-state.svelte';

  type Props = {
    paddingLeft?: number;
    paddingRight?: number;
    paddingTop?: number;
    paddingBottom?: number;
    onnode: (n: NodeInstance) => void;
  };

  const padding = 10;

  const {
    paddingLeft = padding,
    paddingRight = padding,
    paddingTop = padding,
    paddingBottom = padding,
    onnode
  }: Props = $props();

  const graph = getGraphManager();
  const graphState = getGraphState();

  let input: HTMLInputElement;
  let value = $state<string>();
  let activeNodeId = $state<NodeId>();

  const MENU_WIDTH = 150;
  const MENU_HEIGHT = 350;

  const allNodes = graphState.activeSocket
    ? graph.getPossibleNodes(graphState.activeSocket)
    : graph.getNodeDefinitions();

  function filterNodes() {
    return allNodes.filter((node) => node.id.includes(value ?? ''));
  }

  const nodes = $derived(value === '' ? allNodes : filterNodes());
  $effect(() => {
    if (nodes) {
      if (activeNodeId === undefined) {
        activeNodeId = nodes?.[0]?.id;
      } else if (nodes.length) {
        const node = nodes.find((node) => node.id === activeNodeId);
        if (!node) {
          activeNodeId = nodes[0].id;
        }
      }
    }
  });

  function handleNodeCreation(nodeType: NodeInstance['type']) {
    if (!graphState.addMenuPosition) return;
    onnode?.({
      id: -1,
      type: nodeType,
      position: [...graphState.addMenuPosition],
      props: {},
      state: {}
    });
  }

  function handleKeyDown(event: KeyboardEvent) {
    event.stopImmediatePropagation();

    if (event.key === 'Escape') {
      graphState.addMenuPosition = null;
      return;
    }

    if (event.key === 'ArrowDown') {
      const index = nodes.findIndex((node) => node.id === activeNodeId);
      activeNodeId = nodes[(index + 1) % nodes.length].id;
      return;
    }

    if (event.key === 'ArrowUp') {
      const index = nodes.findIndex((node) => node.id === activeNodeId);
      activeNodeId = nodes[(index - 1 + nodes.length) % nodes.length].id;
      return;
    }

    if (event.key === 'Enter') {
      if (activeNodeId && graphState.addMenuPosition) {
        handleNodeCreation(activeNodeId);
      }
      return;
    }
  }

  function clampAddMenuPosition() {
    if (!graphState.addMenuPosition) return;

    const camX = graphState.cameraPosition[0];
    const camY = graphState.cameraPosition[1];
    const zoom = graphState.cameraPosition[2];

    const halfViewportWidth = (graphState.width / 2) / zoom;
    const halfViewportHeight = (graphState.height / 2) / zoom;

    const halfMenuWidth = (MENU_WIDTH / 2) / zoom;
    const halfMenuHeight = (MENU_HEIGHT / 2) / zoom;

    const minX = camX - halfViewportWidth - halfMenuWidth + paddingLeft / zoom;
    const maxX = camX + halfViewportWidth - halfMenuWidth - paddingRight / zoom;
    const minY = camY - halfViewportHeight - halfMenuHeight + paddingTop / zoom;
    const maxY = camY + halfViewportHeight - halfMenuHeight - paddingBottom / zoom;

    const clampedX = Math.max(
      minX + halfMenuWidth,
      Math.min(graphState.addMenuPosition[0], maxX - halfMenuWidth)
    );
    const clampedY = Math.max(
      minY + halfMenuHeight,
      Math.min(graphState.addMenuPosition[1], maxY - halfMenuHeight)
    );

    if (clampedX !== graphState.addMenuPosition[0] || clampedY !== graphState.addMenuPosition[1]) {
      graphState.addMenuPosition = [clampedX, clampedY];
    }
  }

  $effect(() => {
    const pos = graphState.addMenuPosition;
    const zoom = graphState.cameraPosition[2];
    const width = graphState.width;
    const height = graphState.height;

    if (pos && zoom && width && height) {
      clampAddMenuPosition();
    }
  });

  onMount(() => {
    input.disabled = false;
    setTimeout(() => input.focus(), 50);
  });
</script>

<HTML
  position.x={graphState.addMenuPosition?.[0]}
  position.z={graphState.addMenuPosition?.[1]}
  transform={false}
>
  <div class="add-menu-wrapper">
    <div class="header">
      <input
        id="add-menu"
        type="text"
        aria-label="Search for a node type"
        role="searchbox"
        placeholder="Search..."
        disabled={false}
        onkeydown={handleKeyDown}
        bind:value
        bind:this={input}
      />
    </div>

    <div class="content">
      {#each nodes as node (node.id)}
        <div
          class="result"
          role="treeitem"
          tabindex="0"
          aria-selected={node.id === activeNodeId}
          onkeydown={(event) => {
            if (event.key === 'Enter') {
              handleNodeCreation(node.id);
            }
          }}
          onmousedown={() => handleNodeCreation(node.id)}
          onfocus={() => {
            activeNodeId = node.id;
          }}
          class:selected={node.id === activeNodeId}
          onmouseover={() => {
            activeNodeId = node.id;
          }}
        >
          {node.id.split('/').at(-1)}
        </div>
      {/each}
    </div>
  </div>
</HTML>

<style>
  .header {
    padding: 5px;
  }

  input {
    background: var(--color-layer-0);
    font-family: var(--font-family);
    border: none;
    border-radius: 5px;
    color: var(--text-color);
    padding: 0.6em;
    width: calc(100% - 2px);
    box-sizing: border-box;
    font-size: 0.8em;
    margin-left: 1px;
    margin-top: 1px;
  }

  input:focus {
    outline: solid 2px rgba(255, 255, 255, 0.2);
  }

  .add-menu-wrapper {
    position: absolute;
    background: var(--color-layer-1);
    border-radius: 7px;
    overflow: hidden;
    border: solid 2px var(--color-layer-2);
    width: 150px;
  }
  .content {
    min-height: none;
    width: 100%;
    color: var(--text-color);
    max-height: 300px;
    overflow-y: auto;
  }

  .result {
    padding: 1em 0.9em;
    border-bottom: solid 1px var(--color-layer-2);
    opacity: 0.7;
    font-size: 0.9em;
    cursor: pointer;
  }

  .result[aria-selected="true"] {
    background: var(--color-layer-2);
    opacity: 1;
  }
</style>
