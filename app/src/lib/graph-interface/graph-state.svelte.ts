import { animate, lerp } from '$lib/helpers';
import type { NodeInstance, Socket } from '@nodarium/types';
import { getContext, setContext } from 'svelte';
import { SvelteMap, SvelteSet } from 'svelte/reactivity';
import type { OrthographicCamera, Vector3 } from 'three';
import type { GraphManager } from './graph-manager.svelte';
import { ColorGenerator } from './graph/colors';
import { getNodeHeight, getParameterHeight } from './helpers/nodeHelpers';

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

const predefinedColors = {
  path: {
    hue: 80,
    lightness: 20,
    saturation: 80
  },
  float: {
    hue: 70,
    lightness: 10,
    saturation: 0
  },
  geometry: {
    hue: 0,
    lightness: 50,
    saturation: 70
  },
  '*': {
    hue: 200,
    lightness: 20,
    saturation: 100
  }
} as const;

export class GraphState {
  colors = new ColorGenerator(predefinedColors);

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
  safePadding = $state<{ left?: number; right?: number; bottom?: number; top?: number } | null>(
    null
  );
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

  groupSelectedNodes() {
    return this.graph.groupNodes([...this.selectedNodes.keys(), this.activeNodeId]);
  }

  centerNode(node?: NodeInstance) {
    const average = [0, 0, 4];
    if (node) {
      average[0] = node.position[0] + (this.safePadding?.right || 0) / 10;
      average[1] = node.position[1];
      average[2] = 10;
    } else {
      for (const node of this.graph.nodes.values()) {
        average[0] += node.position[0];
        average[1] += node.position[1];
      }
      average[0] = (average[0] / this.graph.nodes.size)
        + (this.safePadding?.right || 0) / (average[2] * 2);
      average[1] /= this.graph.nodes.size;
    }

    const camX = this.cameraPosition[0];
    const camY = this.cameraPosition[1];
    const camZ = this.cameraPosition[2];

    const ease = (t: number) => (t < 0.5 ? 2 * t * t : -1 + (4 - 2 * t) * t);
    const easeZoom = (t: number) => t * t * (3 - 2 * t);

    animate(500, (a: number) => {
      this.cameraPosition[0] = lerp(camX, average[0], ease(a));
      this.cameraPosition[1] = lerp(camY, average[1], ease(a));
      this.cameraPosition[2] = lerp(camZ, average[2], easeZoom(a));
      if (this.mouseDown) return false;
    });
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
          position = this.getSocketPosition(node, index);
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
          position: this.getSocketPosition(node, index)
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
          const nodeType = this.graph.getNodeType(node);
          const height = nodeType ? getNodeHeight(nodeType) : 20;
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
    if (!node) return false;
    const height = getNodeHeight(this.graph.getNodeType(node)!);
    const width = 20;
    return node.position[0] > this.cameraBounds[0] - width
      && node.position[0] < this.cameraBounds[1]
      && node.position[1] > this.cameraBounds[2] - height
      && node.position[1] < this.cameraBounds[3];
  }

  openNodePalette() {
    this.addMenuPosition = [this.mousePosition[0], this.mousePosition[1]];
  }

  enterGroupNode() {
    if (this.activeNodeId === -1) return;
    const node = this.graph.getNode(this.activeNodeId);
    if (!node || node.type !== '__internal/group/instance') return;
    const ok = this.graph.enterGroup(this.activeNodeId, [...this.cameraPosition]);
    if (ok) {
      this.activeNodeId = -1;
      this.clearSelection();
    }
  }

  exitGroupNode() {
    const result = this.graph.exitGroup();
    if (!result) return;
    this.cameraPosition = result.camera;
    this.activeNodeId = result.nodeId;
    this.clearSelection();
  }

  getSocketPosition(
    node: NodeInstance,
    index: string | number
  ): [number, number] {
    if (node.type === '__internal/group/input' && typeof index === 'number') {
      return [
        (node?.state?.x ?? node.position[0]) + 20,
        (node?.state?.y ?? node.position[1]) + 2.5 + 5 * index + 5
      ];
    }

    if (typeof index === 'number') {
      return [
        (node?.state?.x ?? node.position[0]) + 20,
        (node?.state?.y ?? node.position[1]) + 2.5 + 10 * index
      ];
    } else {
      let height = 5;
      const nodeType = this.graph.getNodeType(node)!;
      const inputs = nodeType.inputs || {};
      for (const inputKey in inputs) {
        const h = getParameterHeight(nodeType, inputKey) / 10;
        if (inputKey === index) {
          height += h / 2;
          break;
        }
        height += h;
      }
      return [
        node?.state?.x ?? node.position[0],
        (node?.state?.y ?? node.position[1]) + height
      ];
    }
  }
}
