<script lang="ts">
  import type { NodeId, NodeInstance } from '@nodarium/types';
  import { HTML } from '@threlte/extras';
  import { onDestroy } from 'svelte';
  import type { Attachment } from 'svelte/attachments';
  import { getGraphManager, getGraphState } from '../graph-state.svelte';
  import { groupNodes, nodeLabel, searchNodes, sortByLabel } from '../helpers/addMenu';

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

  let value = $state('');
  let pickedNodeId = $state<NodeId>();

  const MENU_WIDTH = 220;
  const MENU_HEIGHT = 470;

  const allNodes = sortByLabel(
    graphState.activeSocket
      ? graph.getPossibleNodes(graphState.activeSocket)
      : graph.getNodeDefinitions()
  );

  const sections = $derived(groupNodes(searchNodes(allNodes, value)));
  const nodes = $derived(sections.flatMap((section) => section.nodes));
  const activeNodeId = $derived(
    nodes.some((node) => node.id === pickedNodeId) ? pickedNodeId : nodes[0]?.id
  );

  const scrollWhenActive = (id: NodeId): Attachment<HTMLElement> => (el) => {
    if (id === activeNodeId) el.scrollIntoView({ block: 'nearest' });
  };

  let created = false;

  // the query itself is text a user typed and stays here, only its length goes out
  onDestroy(() => {
    if (!created && value && nodes.length === 0) {
      graph.report({ name: 'add-menu-no-results', data: { queryLength: value.length } });
    }
  });

  function handleNodeCreation(nodeType: NodeInstance['type']) {
    if (!graphState.addMenuPosition) return;
    created = true;
    graph.report({
      name: 'node-add',
      data: {
        type: nodeType,
        via: graphState.activeSocket ? 'socket' : value ? 'search' : 'browse'
      }
    });
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
      event.preventDefault();
      const index = nodes.findIndex((node) => node.id === activeNodeId);
      pickedNodeId = nodes[(index + 1) % nodes.length]?.id;
      return;
    }

    if (event.key === 'ArrowUp') {
      event.preventDefault();
      const index = nodes.findIndex((node) => node.id === activeNodeId);
      pickedNodeId = nodes[(index - 1 + nodes.length) % nodes.length]?.id;
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

    const camX = graphState.camera.position[0];
    const camY = graphState.camera.position[1];
    const zoom = graphState.camera.position[2];

    const halfViewportWidth = (graphState.camera.width / 2) / zoom;
    const halfViewportHeight = (graphState.camera.height / 2) / zoom;

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
    const zoom = graphState.camera.position[2];
    const width = graphState.camera.width;
    const height = graphState.camera.height;

    if (pos && zoom && width && height) {
      clampAddMenuPosition();
    }
  });

  const focusOnMount: Attachment<HTMLInputElement> = (el) => {
    const timeout = setTimeout(() => el.focus(), 50);
    return () => clearTimeout(timeout);
  };
</script>

<!-- the nodes are an HTML element of their own, the menu has to stay above them -->
<HTML
  position.x={graphState.addMenuPosition?.[0]}
  position.z={graphState.addMenuPosition?.[1]}
  transform={false}
  zIndexRange={[16777272, 16777272]}
>
  <div class="add-menu-wrapper">
    <div class="header">
      <input
        id="add-menu"
        type="text"
        aria-label="Search for a node type"
        role="searchbox"
        placeholder="Search..."
        autocomplete="off"
        spellcheck="false"
        onkeydown={handleKeyDown}
        oninput={() => (pickedNodeId = undefined)}
        bind:value
        {@attach focusOnMount}
      />
    </div>

    <div class="content" role="tree" aria-label="Node types">
      {#each sections as section (section.id)}
        <div role="group" aria-label={section.title}>
          <div class="section-title" aria-hidden="true"><span>{section.title}</span></div>
          {#each section.nodes as node (node.id)}
            <div
              class="result"
              role="treeitem"
              tabindex="-1"
              aria-selected={node.id === activeNodeId}
              onmousedown={() => handleNodeCreation(node.id)}
              onpointermove={() => (pickedNodeId = node.id)}
              {@attach scrollWhenActive(node.id)}
            >
              {nodeLabel(node)}
            </div>
          {/each}
        </div>
      {:else}
        <div class="no-results">No results for "{value}"</div>
      {/each}
    </div>
  </div>
</HTML>

<style>
  .header {
    padding: 5px;
    border-bottom: solid 1px var(--color-layer-2);
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
    width: 220px;
  }

  .content {
    width: 100%;
    color: var(--text-color);
    max-height: 420px;
    overflow-y: auto;
    overscroll-behavior: contain;
  }

  .section-title {
    position: sticky;
    top: 0;
    z-index: 1;
    padding: 0.7em 0.9em 0.35em;
    background: var(--color-layer-1);
    border-bottom: solid 1px var(--color-layer-2);
    font-size: 0.75em;
  }

  /* on the title itself, the opacity would let the rows show through its background */
  .section-title span {
    opacity: 0.5;
  }

  .result {
    padding: 0.45em 0.9em;
    opacity: 0.7;
    font-size: 0.85em;
    cursor: pointer;
    /* the sticky title covers a row scrolled to the top otherwise */
    scroll-margin-top: 2.2em;
  }

  .result[aria-selected="true"] {
    background: var(--color-layer-2);
    opacity: 1;
  }

  .no-results {
    padding: 1em 0.9em;
    font-size: 0.85em;
    opacity: 0.45;
    font-style: italic;
  }
</style>
