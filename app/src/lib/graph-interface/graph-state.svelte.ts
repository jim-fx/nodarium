import { GROUP_INSTANCE } from '@nodarium/types';
import type { Frame, NodeInstance, SerializedEdge, SerializedNode, Socket } from '@nodarium/types';
import { createContext } from 'svelte';
import { SvelteSet } from 'svelte/reactivity';
import type { Vector3 } from 'three';
import { GraphCamera } from './camera.svelte';
import type { GraphManager } from './graph-manager.svelte';
import { ColorGenerator } from './graph/colors';
import { maxZoom } from './graph/constants';
import { serializeFrame } from './helpers/frames';
import { animate, easeInOut, lerp } from './helpers/index.ts';
import { NODE_WIDTH } from './helpers/layoutGraph';
import { boxContains, boxesOverlap, nodeBox } from './helpers/nodeBox';
import {
  getInputSocketOffset,
  getOutputSocketOffset,
  positionOf,
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

type PointerPosition = Pick<MouseEvent, 'clientX' | 'clientY'>;
const NO_NODES: ReadonlySet<number> = new Set();

type EdgeData = {
  x1: number;
  y1: number;
  points: Vector3[];
};

export class GraphState {
  colors = new ColorGenerator();

  constructor(private graph: GraphManager, private readOptions: () => GraphOptions) {}

  camera = new GraphCamera();

  get options() {
    return this.readOptions();
  }

  /** Set by the wrapper, the nodes to point out apart from the selection */
  readHighlightedNodes: () => ReadonlySet<number> = () => NO_NODES;

  isHighlighted(nodeId: number) {
    return this.readHighlightedNodes().has(nodeId);
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
    frames: Frame[];
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
  /** The frame whose title was clicked, -1 for none. A frame is selected without its nodes. */
  activeFrameId = $state(-1);
  get activeFrame() {
    return this.activeFrameId === -1 ? undefined : this.graph.getFrame(this.activeFrameId);
  }
  /** The frame whose title is being renamed */
  renamingFrameId = $state(-1);
  activeSocket = $state<Socket | null>(null);
  hoveredSocket = $state<Socket | null>(null);
  possibleSockets = $state<Socket[]>([]);
  possibleSocketIds = $derived(
    new SvelteSet(this.possibleSockets.map((s) => `${s.node.id}-${s.index}`))
  );
  /**
   * The socket a click (or Enter) picked to connect from without a drag. The sockets it can
   * connect to stay highlighted until one of them is clicked.
   */
  pickedSocket = $state.raw<Socket | null>(null);
  /** Where the pointer pressed a socket, to tell a click from a drag */
  private socketPressedAt: [number, number] | null = null;

  isPicked(node: NodeInstance, index: number | string) {
    return this.pickedSocket?.node.id === node.id && this.pickedSocket.index === index;
  }

  /** The socket an edge is dragged or picked from */
  get connectingSocket() {
    return this.activeSocket ?? this.pickedSocket;
  }

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

  /** The active node and the selected nodes, or the nodes of the selected frame */
  selectedNodeIds(): number[] {
    if (this.activeFrame) return [...this.activeFrame.nodes];
    const ids = new SvelteSet([this.activeNodeId, ...this.selectedNodes]);
    ids.delete(-1);
    return [...ids];
  }

  /** Selects the frame alone, without its nodes */
  selectFrame(id: number) {
    this.activeNodeId = -1;
    this.clearSelection();
    this.activeFrameId = id;
  }

  frameSelection() {
    if (this.activeFrame) return;
    const frame = this.graph.createFrame(this.selectedNodeIds());
    if (frame) this.selectFrame(frame.id);
  }

  copyNodes() {
    const ids = new SvelteSet(this.selectedNodeIds());
    if (!ids.size) return;
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

    const frames = this.graph.frames.filter((f) => f.nodes.every((id) => ids.has(id)));

    this.clipboard = {
      nodes: nodes.map(n => serializeNode(n)),
      edges: edges.map(e => serializeEdge(e)),
      frames: frames.map(serializeFrame)
    };
  }

  unGroupSelectedNodes() {
    return this.graph.ungroupNode(this.activeNodeId);
  }

  groupSelectedNodes() {
    return this.graph.groupNodes([...this.selectedNodes.keys(), this.activeNodeId]);
  }

  // Fits the node, the nodes of the selected frame, or all nodes into the part of the view the
  // safe padding leaves free
  centerNode(node?: NodeInstance) {
    const frameNodes = this.activeFrame?.nodes.flatMap((id) => this.graph.getNode(id) ?? []);
    const nodes = node ? [node] : frameNodes ?? [...this.graph.nodes.values()];
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
        this.mousePosition[0] - positionOf(node)[0],
        this.mousePosition[1] - positionOf(node)[1]
      ] as [number, number]
    }));

    const newNodes = this.graph.createGraph(nodes, this.clipboard.edges, this.clipboard.frames);
    this.activeFrameId = -1;
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

    this.possibleSockets = this.socketsToConnect(this.activeSocket);
  }

  private socketsToConnect(socket: Socket): Socket[] {
    return this.graph.getPossibleSockets(socket).map(([node, index]) => ({
      node,
      index,
      position: this.getSocketPosition(node, index)
    }));
  }

  pickSocket(socket: Socket) {
    this.pickedSocket = socket;
    this.possibleSockets = this.socketsToConnect(socket);
  }

  unpickSocket() {
    if (!this.pickedSocket) return;
    this.pickedSocket = null;
    this.possibleSockets = [];
  }

  private isPickedSocket(socket: Socket) {
    return this.isPicked(socket.node, socket.index);
  }

  /**
   * Connects the picked socket to `socket` if that is one of the highlighted sockets, and
   * leaves the picked state. Returns false if `socket` is not highlighted.
   */
  private connectPicked(socket: Socket): boolean {
    const picked = this.pickedSocket;
    const fits = this.possibleSockets.some((s) =>
      s.node.id === socket.node.id && s.index === socket.index
    );
    if (!picked || !fits) return false;
    this.unpickSocket();
    const [from, to] = typeof picked.index === 'number' ? [picked, socket] : [socket, picked];
    // the picked node may be gone or replaced since, e.g. by an undo
    const fromNode = this.graph.getNode(from.node.id);
    const toNode = this.graph.getNode(to.node.id);
    if (fromNode && toNode) {
      this.graph.createEdge(fromNode, from.index as number, toNode, to.index as string);
    }
    return true;
  }

  /**
   * A mouse press on a socket: connects to the picked socket, or starts dragging an edge as
   * before. Released without a drag, the press picks the socket, see releasedSocket().
   */
  pressSocket(socket: Socket, event: PointerPosition) {
    this.socketPressedAt = null;
    if (this.connectPicked(socket)) return;
    const wasPicked = this.isPickedSocket(socket);
    this.unpickSocket();
    this.setDownSocket(socket);
    // a press on the picked socket unpicks it, and a press on a connected input drags its edge
    // away: neither picks
    const dragsOwnSocket = this.activeSocket?.node.id === socket.node.id
      && this.activeSocket.index === socket.index;
    if (dragsOwnSocket && !wasPicked) this.socketPressedAt = [event.clientX, event.clientY];
  }

  /** A press that moves further than a click turns into a drag */
  movePointer(event: PointerPosition) {
    const at = this.socketPressedAt;
    if (at && Math.hypot(event.clientX - at[0], event.clientY - at[1]) > 4) {
      this.socketPressedAt = null;
    }
  }

  /** The socket the mouse pressed and released without dragging, see pressSocket() */
  releasedSocket(event: PointerPosition): Socket | null {
    this.movePointer(event);
    const pressed = this.socketPressedAt && this.activeSocket;
    this.socketPressedAt = null;
    return pressed || null;
  }

  /** Enter or Space on a socket: connects to the picked socket, or picks it, or unpicks it */
  activateSocket(socket: Socket) {
    if (this.connectPicked(socket)) return;
    const wasPicked = this.isPickedSocket(socket);
    this.unpickSocket();
    if (!wasPicked) this.pickSocket(socket);
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

  enterGroupNode(): boolean {
    if (this.activeNodeId === -1) return false;
    const node = this.graph.getNode(this.activeNodeId);
    if (!node || node.type !== GROUP_INSTANCE) return false;
    const ok = this.graph.enterGroup(this.activeNodeId);
    if (ok) {
      this.activeNodeId = -1;
      this.clearSelection();
    }
    return !!ok;
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
