import { GROUP_INSTANCE } from '@nodarium/types';
import type { NodeInstance, SerializedEdge, SerializedNode, Socket } from '@nodarium/types';
import { createContext } from 'svelte';
import { SvelteSet } from 'svelte/reactivity';
import type { Vector3 } from 'three';
import { GraphCamera } from './camera.svelte';
import type { GraphManager } from './graph-manager.svelte';
import { ColorGenerator } from './graph/colors';
import { maxZoom } from './graph/constants';
import { animate, easeInOut, lerp } from './helpers/index.ts';
import { NODE_WIDTH } from './helpers/layoutGraph';
import { boxContains, boxesOverlap, nodeBox } from './helpers/nodeBox';
import {
  getInputSocketOffset,
  getOutputSocketOffset,
  serializeEdge,
  serializeNode
} from './helpers/nodeHelpers';
import type { GraphOptions } from './options';

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

  constructor(private graph: GraphManager, private readOptions: () => GraphOptions) {}

  camera = new GraphCamera();

  get options() {
    return this.readOptions();
  }

  hoveredEdgeId = $state<string | null>(null);
  /** Not reactive: written by the edges when they are drawn, read by the pointer */
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  edgeGeometry = new Map<string, EdgeData>();

  wrapper = $state<HTMLDivElement>(null!);
  rect: DOMRect = $derived(
    (this.wrapper && this.camera.width && this.camera.height)
      ? this.wrapper.getBoundingClientRect()
      : new DOMRect(0, 0, 0, 0)
  );

  clipboard: null | {
    nodes: SerializedNode[];
    edges: SerializedEdge[];
  } = null;

  boxSelection = $state(false);
  edgeEndPosition = $state<[number, number] | null>();
  addMenuPosition = $state<[number, number] | null>(null);

  cameraDown: [number, number] = [0, 0];
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

  clearSelection() {
    this.selectedNodes.clear();
  }

  isBodyFocused = () => document?.activeElement?.nodeName !== 'INPUT';

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

  // Fits the node (or all nodes) into the part of the view the safe padding leaves free
  centerNode(node?: NodeInstance) {
    const nodes = node ? [node] : [...this.graph.nodes.values()];
    if (!nodes.length) return;

    const box = { minX: Infinity, maxX: -Infinity, minY: Infinity, maxY: -Infinity };
    for (const n of nodes) {
      const b = nodeBox(n, this.graph.getNodeType(n));
      box.minX = Math.min(box.minX, b.minX);
      box.maxX = Math.max(box.maxX, b.maxX);
      box.minY = Math.min(box.minY, b.minY);
      box.maxY = Math.max(box.maxY, b.maxY);
    }

    const pad = this.options.safePadding;
    const margin = 40;
    const padding = {
      left: (pad?.left || 0) + margin,
      right: (pad?.right || 0) + margin,
      top: (pad?.top || 0) + margin,
      bottom: (pad?.bottom || 0) + margin
    };

    // a single node should not be blown up to fill the whole view
    const target = this.camera.fit(box, padding, nodes.length === 1 ? 10 : maxZoom);
    this.camera.animateTo(target, () => !!this.mouseDown);
  }

  // lays out the nodes and lets them glide to their new positions
  formatNodes() {
    const nodes = [...this.graph.nodes.values()];
    const from = nodes.map((node) => [
      node.state.x ?? node.position[0],
      node.state.y ?? node.position[1]
    ]);

    this.graph.formatNodes();

    animate(500, (a: number) => {
      const t = easeInOut(a);
      nodes.forEach((node, i) => {
        node.state.x = a === 1 ? node.position[0] : lerp(from[i][0], node.position[0], t);
        node.state.y = a === 1 ? node.position[1] : lerp(from[i][1], node.position[1], t);
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

  getNodeIdFromEvent(event: MouseEvent) {
    let clickedNodeId = -1;

    const mx = event.clientX - this.rect.x;
    const my = event.clientY - this.rect.y;

    if (event.button === 0 || event.button === 2) {
      if (event.target instanceof HTMLElement) {
        const nodeElement = event.target.closest('.node');
        const nodeId = nodeElement?.getAttribute?.('data-node-id');
        if (nodeId) {
          clickedNodeId = parseInt(nodeId, 10);
        }
      }

      if (clickedNodeId === -1) {
        const [downX, downY] = this.camera.toWorld(mx, my);
        for (const node of this.graph.nodes.values()) {
          if (boxContains(nodeBox(node, this.graph.getNodeType(node)), downX, downY)) {
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
    return boxesOverlap(nodeBox(node, this.graph.getNodeType(node)), this.camera.bounds);
  }

  openNodePalette() {
    this.addMenuPosition = [this.mousePosition[0], this.mousePosition[1]];
  }

  enterGroupNode() {
    if (this.activeNodeId === -1) return;
    const node = this.graph.getNode(this.activeNodeId);
    if (!node || node.type !== GROUP_INSTANCE) return;
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
        (node?.state?.x ?? node.position[0]) + NODE_WIDTH,
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
