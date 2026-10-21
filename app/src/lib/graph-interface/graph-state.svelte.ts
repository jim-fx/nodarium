import { animate, debounce, lerp } from '#lib/helpers/index.ts';
import type { NodeInstance, SerializedEdge, SerializedNode, Socket } from '@nodarium/types';
import { createContext } from 'svelte';
import { SvelteMap, SvelteSet } from 'svelte/reactivity';
import type { OrthographicCamera, Vector3 } from 'three';
import type { GraphManager } from './graph-manager.svelte';
import { ColorGenerator } from './graph/colors';
import { maxZoom, minZoom } from './graph/constants';
import {
  getInputSocketOffset,
  getNodeHeight,
  getOutputSocketOffset,
  serializeEdge,
  serializeNode
} from './helpers/nodeHelpers';

const [getGraphState, setGraphState, hasGraphState] = createContext<GraphState>();
const [getGraphManager, setGraphManager, hasGraphManager] = createContext<GraphManager>();

// For node components that are also rendered outside of a graph, e.g. in the node store.
// Typed as always present because everything that needs them only runs inside a graph.
const tryGetGraphState = () => (hasGraphState() ? getGraphState() : undefined) as GraphState;
const tryGetGraphManager = () =>
  (hasGraphManager() ? getGraphManager() : undefined) as GraphManager;

export {
  getGraphManager,
  getGraphState,
  setGraphManager,
  setGraphState,
  tryGetGraphManager,
  tryGetGraphState
};

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
    const saveCameraPosition = debounce(() => {
      localStorage.setItem(
        'cameraPosition',
        `[${this.cameraPosition[0]},${this.cameraPosition[1]},${this.cameraPosition[2]}]`
      );
    }, 500);

    $effect.root(() => {
      $effect(() => {
        // Read values to subscribe to reactivity, then flush lazily.
        void this.cameraPosition[0];
        void this.cameraPosition[1];
        void this.cameraPosition[2];
        saveCameraPosition();
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
    nodes: SerializedNode[];
    edges: SerializedEdge[];
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
  nodeTimings = $state<Record<number, number>>({});
  /** Node that made the last execution fail */
  errorNodeId = $state<number>();

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

  private _dirtyPositions = new SvelteSet<NodeInstance>();
  private _positionFlushPending = false;

  private _flushPositions() {
    for (const node of this._dirtyPositions) {
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
    this._dirtyPositions.clear();
    this._positionFlushPending = false;
  }

  updateNodePosition(node: NodeInstance) {
    if (
      node.state.x === node.position[0]
      && node.state.y === node.position[1]
    ) {
      delete node.state.x;
      delete node.state.y;
    }

    this._dirtyPositions.add(node);
    if (!this._positionFlushPending) {
      this._positionFlushPending = true;
      requestAnimationFrame(() => this._flushPositions());
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
    const ids = new SvelteSet([this.activeNodeId, ...(this.selectedNodes?.values() || [])]);
    let nodes = [...ids]
      .map((id) => this.graph.getNode(id))
      .filter((b): b is NodeInstance => !!b);

    const edges = this.graph.getEdgesBetweenNodes(nodes);
    nodes = nodes.map((node) => ({
      ...node,
      position: [
        this.mousePosition[0] - node.position[0],
        this.mousePosition[1] - node.position[1]
      ]
    }));

    this.clipboard = {
      nodes: nodes.map(n => serializeNode(n)),
      edges: edges.map(e => serializeEdge(e))
    };
  }

  unGroupSelectedNodes() {
    return this.graph.ungroupNode(this.activeNodeId);
  }

  groupSelectedNodes() {
    return this.graph.groupNodes([...this.selectedNodes.keys(), this.activeNodeId]);
  }

  // Computes the camera [x, y, zoom] that fits the bounding box of the given nodes
  // into the part of the view that is not covered by the safe padding.
  private getFitCamera(nodes: NodeInstance[]): [number, number, number] | undefined {
    if (!nodes.length) return;

    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (const node of nodes) {
      const type = this.graph.getNodeType(node);
      const x = node.state.x ?? node.position[0];
      const y = node.state.y ?? node.position[1];
      minX = Math.min(minX, x);
      minY = Math.min(minY, y);
      maxX = Math.max(maxX, x + 20);
      maxY = Math.max(maxY, y + (type ? getNodeHeight(type) : 20));
    }

    const pad = this.safePadding;
    const margin = 40; // px of breathing room around the box
    const left = (pad?.left || 0) + margin;
    const right = (pad?.right || 0) + margin;
    const top = (pad?.top || 0) + margin;
    const bottom = (pad?.bottom || 0) + margin;

    const availableWidth = Math.max(this.width - left - right, 1);
    const availableHeight = Math.max(this.height - top - bottom, 1);

    const boxWidth = maxX - minX;
    const boxHeight = maxY - minY;

    // A single node should not be blown up to fill the whole screen
    const maxFitZoom = nodes.length === 1 ? 10 : maxZoom;
    const zoom = Math.max(
      minZoom,
      Math.min(maxFitZoom, availableWidth / boxWidth, availableHeight / boxHeight)
    );

    // Shift the camera so the box center lands in the center of the available area
    const x = (minX + maxX) / 2 - (left - right) / 2 / zoom;
    const y = (minY + maxY) / 2 - (top - bottom) / 2 / zoom;
    return [x, y, zoom];
  }

  // Fits the node (or all nodes) into the current view
  centerNode(node?: NodeInstance) {
    const target = this.getFitCamera(node ? [node] : [...this.graph.nodes.values()]);
    if (!target) return;

    const from = [...this.cameraPosition];
    const ease = (t: number) => (t < 0.5 ? 2 * t * t : -1 + (4 - 2 * t) * t);

    // Position and zoom share one easing so the view stays consistent while animating
    animate(500, (a: number) => {
      const t = ease(a);
      this.cameraPosition[0] = lerp(from[0], target[0], t);
      this.cameraPosition[1] = lerp(from[1], target[1], t);
      this.cameraPosition[2] = lerp(from[2], target[2], t);
      if (this.mouseDown) return false;
    });
  }

  // lays out the nodes and lets them glide to their new positions
  formatNodes() {
    const nodes = [...this.graph.nodes.values()];
    const from = nodes.map((node) => [
      node.state.x ?? node.position[0],
      node.state.y ?? node.position[1]
    ]);

    this.graph.formatNodes();

    const ease = (t: number) => (t < 0.5 ? 2 * t * t : -1 + (4 - 2 * t) * t);
    animate(500, (a: number) => {
      nodes.forEach((node, i) => {
        node.state.x = a === 1 ? node.position[0] : lerp(from[i][0], node.position[0], ease(a));
        node.state.y = a === 1 ? node.position[1] : lerp(from[i][1], node.position[1], ease(a));
        this.updateNodePosition(node);
      });
    });
  }

  pasteNodes() {
    if (!this.clipboard) return;

    // Create fresh node objects — never mutate clipboard so repeat pastes work correctly.
    // State is also spread (with cleared parents/children) so createGraph's mutations
    // don't corrupt the clipboard's stored state references.
    const nodes = this.clipboard.nodes.map((node) => ({
      ...node,
      position: [
        this.mousePosition[0] - node.position[0],
        this.mousePosition[1] - node.position[1]
      ] as [number, number]
    }));

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

    if (event.button === 0 || event.button === 2) {
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
    const ok = this.graph.enterGroup(this.activeNodeId);
    if (ok) {
      this.activeNodeId = -1;
      this.clearSelection();
    }
  }

  exitGroupNode() {
    const result = this.graph.exitGroup();
    if (!result) return;
    this.activeNodeId = result.nodeId;
    this.clearSelection();
  }

  getSocketPosition(
    node: NodeInstance,
    index: string | number
  ): [number, number] {
    if (typeof index === 'number') {
      return [
        (node?.state?.x ?? node.position[0]) + 20,
        (node?.state?.y ?? node.position[1]) + getOutputSocketOffset(node.type, index)
      ];
    } else {
      // the node type is missing when its wasm failed to load
      const nodeType = this.graph.getNodeType(node);
      return [
        node?.state?.x ?? node.position[0],
        (node?.state?.y ?? node.position[1]) + getInputSocketOffset(nodeType, index)
      ];
    }
  }
}
