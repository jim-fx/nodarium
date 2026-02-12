import type { NodeInstance, Socket } from '@nodarium/types';
import { getContext, setContext } from 'svelte';
import { SvelteMap, SvelteSet } from 'svelte/reactivity';
import type { OrthographicCamera, Vector3 } from 'three';
import type { GraphManager } from './graph-manager.svelte';
import { getNodeHeight, getSocketPosition } from './helpers/nodeHelpers';

const graphStateKey = Symbol('graph-state');
export function getGraphState() {
  return getContext<GraphState>(graphStateKey);
}
export function setGraphState(graphState: GraphState) {
  return setContext(graphStateKey, graphState);
}

const graphManagerKey = Symbol('graph-manager');
export function getGraphManager() {
  return getContext<GraphManager>(graphManagerKey);
}

export function setGraphManager(manager: GraphManager) {
  return setContext(graphManagerKey, manager);
}

type EdgeData = {
  x1: number;
  y1: number;
  points: Vector3[];
};

export class GraphState {
  constructor(private graph: GraphManager) {
    $effect.root(() => {
      $effect(() => {
        localStorage.setItem(
          'cameraPosition',
          `[${this.cameraPosition[0]},${this.cameraPosition[1]},${this.cameraPosition[2]}]`
        );
      });
    });
    const storedPosition = localStorage.getItem('cameraPosition');
    if (storedPosition) {
      try {
        const d = JSON.parse(storedPosition);
        this.cameraPosition[0] = d[0];
        this.cameraPosition[1] = d[1];
        this.cameraPosition[2] = d[2];
      } catch (e) {
        console.log('Failed to parsed stored camera position', e);
      }
    }
  }

  width = $state(100);
  height = $state(100);

  hoveredEdgeId = $state<string | null>(null);
  edges = new SvelteMap<string, EdgeData>();

  wrapper = $state<HTMLDivElement>(null!);
  rect: DOMRect = $derived(
    (this.wrapper && this.width && this.height)
      ? this.wrapper.getBoundingClientRect()
      : new DOMRect(0, 0, 0, 0)
  );

  camera = $state<OrthographicCamera>(null!);
  cameraPosition: [number, number, number] = $state([140, 100, 3.5]);

  clipboard: null | {
    nodes: NodeInstance[];
    edges: [number, number, number, string][];
  } = null;

  cameraBounds = $derived([
    this.cameraPosition[0] - this.width / this.cameraPosition[2] / 2,
    this.cameraPosition[0] + this.width / this.cameraPosition[2] / 2,
    this.cameraPosition[1] - this.height / this.cameraPosition[2] / 2,
    this.cameraPosition[1] + this.height / this.cameraPosition[2] / 2
  ]);

  boxSelection = $state(false);
  edgeEndPosition = $state<[number, number] | null>();
  addMenuPosition = $state<[number, number] | null>(null);

  snapToGrid = $state(false);
  backgroundType = $state<'grid' | 'dots' | 'none'>('grid');
  showHelp = $state(false);

  cameraDown = [0, 0];
  mouseDownNodeId = -1;

  isPanning = $state(false);
  isDragging = $state(false);
  hoveredNodeId = $state(-1);
  mousePosition = $state([0, 0]);
  mouseDown = $state<[number, number] | null>(null);
  activeNodeId = $state(-1);
  selectedNodes = new SvelteSet<number>();
  activeSocket = $state<Socket | null>(null);
  hoveredSocket = $state<Socket | null>(null);
  possibleSockets = $state<Socket[]>([]);
  possibleSocketIds = $derived(
    new SvelteSet(this.possibleSockets.map((s) => `${s.node.id}-${s.index}`))
  );

  getEdges() {
    return $state.snapshot(this.edges);
  }

  clearSelection() {
    this.selectedNodes.clear();
  }

  isBodyFocused = () => document?.activeElement?.nodeName !== 'INPUT';

  setEdgeGeometry(edgeId: string, x1: number, y1: number, points: Vector3[]) {
    this.edges.set(edgeId, { x1, y1, points });
  }

  removeEdgeGeometry(edgeId: string) {
    this.edges.delete(edgeId);
  }

  getEdgeData() {
    return this.edges;
  }

  updateNodePosition(node: NodeInstance) {
    if (
      node.state.x === node.position[0]
      && node.state.y === node.position[1]
    ) {
      delete node.state.x;
      delete node.state.y;
    }

    if (node.state['x'] !== undefined && node.state['y'] !== undefined) {
      if (node.state.ref) {
        node.state.ref.style.setProperty('--nx', `${node.state.x * 10}px`);
        node.state.ref.style.setProperty('--ny', `${node.state.y * 10}px`);
      }
    } else {
      if (node.state.ref) {
        node.state.ref.style.setProperty('--nx', `${node.position[0] * 10}px`);
        node.state.ref.style.setProperty('--ny', `${node.position[1] * 10}px`);
      }
    }
  }

  getSnapLevel() {
    const z = this.cameraPosition[2];
    if (z > 66) {
      return 8;
    } else if (z > 55) {
      return 4;
    } else if (z > 11) {
      return 2;
    }
    return 1;
  }

  tryConnectToDebugNode(nodeId: number) {
    const node = this.graph.nodes.get(nodeId);
    if (!node) return;
    if (node.type.endsWith('/debug')) return;
    if (!node.state.type?.outputs?.length) return;
    for (const _node of this.graph.nodes.values()) {
      if (_node.type.endsWith('/debug')) {
        this.graph.createEdge(node, 0, _node, 'input');
        return;
      }
    }

    const debugNode = this.graph.createNode({
      type: 'max/plantarium/debug',
      position: [node.position[0] + 30, node.position[1]],
      props: {}
    });

    if (debugNode) {
      this.graph.createEdge(node, 0, debugNode, 'input');
    }
  }

  copyNodes() {
    if (this.activeNodeId === -1 && !this.selectedNodes?.size) {
      return;
    }
    let nodes = [
      this.activeNodeId,
      ...(this.selectedNodes?.values() || [])
    ]
      .map((id) => this.graph.getNode(id))
      .filter(b => !!b);

    const edges = this.graph.getEdgesBetweenNodes(nodes);
    nodes = nodes.map((node) => ({
      ...node,
      position: [
        this.mousePosition[0] - node.position[0],
        this.mousePosition[1] - node.position[1]
      ],
      tmp: undefined
    }));

    this.clipboard = {
      nodes: nodes,
      edges: edges
    };
  }

  pasteNodes() {
    if (!this.clipboard) return;

    const nodes = this.clipboard.nodes
      .map((node) => {
        node.position[0] = this.mousePosition[0] - node.position[0];
        node.position[1] = this.mousePosition[1] - node.position[1];
        return node;
      })
      .filter(Boolean) as NodeInstance[];

    const newNodes = this.graph.createGraph(nodes, this.clipboard.edges);
    this.selectedNodes.clear();
    for (const node of newNodes) {
      this.selectedNodes.add(node.id);
    }
  }

  setDownSocket(socket: Socket) {
    this.activeSocket = socket;

    let { node, index, position } = socket;

    // remove existing edge
    if (typeof index === 'string') {
      const edges = this.graph.getEdgesToNode(node);
      for (const edge of edges) {
        if (edge[3] === index) {
          node = edge[0];
          index = edge[1];
          position = getSocketPosition(node, index);
          this.graph.removeEdge(edge);
          break;
        }
      }
    }

    this.mouseDown = position;
    this.activeSocket = {
      node,
      index,
      position
    };

    this.possibleSockets = this.graph
      .getPossibleSockets(this.activeSocket)
      .map(([node, index]) => {
        return {
          node,
          index,
          position: getSocketPosition(node, index)
        };
      });
  }

  projectScreenToWorld(x: number, y: number): [number, number] {
    return [
      this.cameraPosition[0]
      + (x - this.width / 2) / this.cameraPosition[2],
      this.cameraPosition[1]
      + (y - this.height / 2) / this.cameraPosition[2]
    ];
  }

  getNodeIdFromEvent(event: MouseEvent) {
    let clickedNodeId = -1;

    const mx = event.clientX - this.rect.x;
    const my = event.clientY - this.rect.y;

    if (event.button === 0) {
      // check if the clicked element is a node
      if (event.target instanceof HTMLElement) {
        const nodeElement = event.target.closest('.node');
        const nodeId = nodeElement?.getAttribute?.('data-node-id');
        if (nodeId) {
          clickedNodeId = parseInt(nodeId, 10);
        }
      }

      // if we do not have an active node,
      // we are going to check if we clicked on a node by coordinates
      if (clickedNodeId === -1) {
        const [downX, downY] = this.projectScreenToWorld(mx, my);
        for (const node of this.graph.nodes.values()) {
          const x = node.position[0];
          const y = node.position[1];
          const height = getNodeHeight(node.state.type!);
          if (downX > x && downX < x + 20 && downY > y && downY < y + height) {
            clickedNodeId = node.id;
            break;
          }
        }
      }
    }
    return clickedNodeId;
  }

  isNodeInView(node: NodeInstance) {
    const height = getNodeHeight(node.state.type!);
    const width = 20;
    return node.position[0] > this.cameraBounds[0] - width
      && node.position[0] < this.cameraBounds[1]
      && node.position[1] > this.cameraBounds[2] - height
      && node.position[1] < this.cameraBounds[3];
  }

  openNodePalette() {
    this.addMenuPosition = [this.mousePosition[0], this.mousePosition[1]];
  }
}
