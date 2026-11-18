import {
  CURRENT_GRAPH_VERSION,
  GraphVersionError,
  migrateGraph,
  placeNodes
} from '@nodarium/runtime';
import {
  DEBUG_NODE,
  getOutputType,
  GROUP_INPUT,
  GROUP_INSTANCE,
  GROUP_OUTPUT,
  groupIdOf,
  inputAccepts,
  INTERNAL_PREFIX,
  isDebugNode,
  isOutputNode
} from '@nodarium/types';
import type {
  Edge,
  Graph,
  GroupDefinition,
  NodeDefinition,
  NodeId,
  NodeInput,
  NodeInstance,
  NodeRegistry,
  SerializedEdge,
  SerializedNode,
  Socket
} from '@nodarium/types';
import { toast } from '@nodarium/ui';
import { createLogger } from '@nodarium/utils';
import { SvelteMap, SvelteSet } from 'svelte/reactivity';
import { ChangeRecorder } from './change-recorder';
import { collectSettings } from './helpers/collectSettings';
import EventEmitter from './helpers/EventEmitter';
import {
  exposeInputs,
  groupInputType,
  groupInstanceType,
  groupOptions,
  groupOutputType
} from './helpers/group-types';
import { groupSelection, ungroupInstance } from './helpers/grouping';
import { toPlain } from './helpers/index.ts';
import { COLUMN_GAP, layoutGraph, NODE_WIDTH } from './helpers/layoutGraph';
import {
  areEdgesEqual,
  getNodeHeight,
  isInputConnectable,
  positionOf,
  serializeEdge,
  serializeNode
} from './helpers/nodeHelpers';
import { buildRelations, reachable } from './helpers/relations';
import throttle from './helpers/throttle.ts';

const log = createLogger('graph-manager');
log.mute();

type Level = { nodes: SerializedNode[]; edges: SerializedEdge[] };

function sameKeys(a: object, b: object) {
  const keys = Object.keys(a);
  return keys.length === Object.keys(b).length && keys.every((key) => key in b);
}

export class GraphManager extends EventEmitter<{
  save: Graph;
  result: Graph;
  /** load() placed nodes that had no position, the camera doesn't know where they are */
  placed: Graph;
}> {
  status = $state<'loading' | 'idle' | 'error'>();

  /** The group instances entered from the root, empty at the root */
  path: { groupId: number; nodeId: number }[] = $state([]);
  currentGroupId = $derived(this.path.at(-1)?.groupId ?? null);
  isInsideGroup = $derived(this.path.length > 0);

  // The root level while a group is open. A closed group is stored in its GroupDefinition.
  private root: Level = $state.raw({ nodes: [], edges: [] });

  id = $state(0);
  meta = $state<Graph['meta']>({});
  nodes = new SvelteMap<number, NodeInstance>();
  edges = $state<Edge[]>([]);
  groups: GroupDefinition[] = $state([]);

  nodeArray = $derived(Array.from(this.nodes.values()));

  /** The settings the nodes of the graph declare, and the seed */
  settingTypes: Record<string, NodeInput> = $state.raw({});
  settings: Record<string, unknown> = $state({});

  /** The edge into each connected input, keyed by `${nodeId}-${inputKey}` */
  inputSockets = $derived.by(() => {
    const s = new SvelteMap<string, Edge>();
    for (const edge of this.edges) {
      s.set(`${edge[2].id}-${edge[3]}`, edge);
    }
    return s;
  });

  private recorder = new ChangeRecorder({
    snapshot: () => {
      this.updateSettings();
      return this.serialized;
    },
    onsave: () => {
      if (this.nodes.size === 0 && this.edges.length === 0) return;
      this.emit('save', this.serialize());
    }
  });

  get history() {
    return this.recorder.history;
  }

  /** A build was asked for or a load is under way, and its `result` is not emitted yet */
  buildPending = $state(false);

  private emitResult = throttle(() => {
    this.buildPending = false;
    if (this.status !== 'idle') return;
    this.emit('result', this.serialize());
  }, 10);

  /** Asks for a build of the graph as it is now */
  execute = () => {
    this.buildPending = true;
    this.emitResult();
  };

  // The changes of the running transactions, recorded once the outermost one ends
  private transactionDepth = 0;
  private pendingChange: { build: boolean; coalesce: boolean } | undefined;
  // Not a counter: an end that never came (the gizmo unmounted mid-drag) must not hold every later step
  private heldStep: { changed: boolean } | undefined;

  /** The graph changed. `build: false` for changes the result doesn't depend on, like a position. */
  commit({ build = true, coalesce = false } = {}) {
    if (this.transactionDepth > 0) {
      this.pendingChange = {
        build: build || !!this.pendingChange?.build,
        coalesce: coalesce && (this.pendingChange?.coalesce ?? true)
      };
      return;
    }
    if (this.heldStep) this.heldStep.changed = true;
    else if (coalesce) this.recorder.coalesce();
    else this.recorder.step();
    if (build) this.execute();
  }

  /** Several changes as one: one undo step and one build, after the last of them */
  transaction<T>(change: () => T): T {
    this.transactionDepth++;
    try {
      return change();
    } finally {
      this.transactionDepth--;
      const pending = this.pendingChange;
      if (this.transactionDepth === 0 && pending) {
        this.pendingChange = undefined;
        this.commit(pending);
      }
    }
  }

  /**
   * A change that takes time, like a drag in the viewer: it builds on the way, the undo step waits
   * for the end. A new begin, undo, redo, flush() and load() end a step that is still held.
   */
  beginUndoStep() {
    this.endUndoStep();
    this.heldStep = { changed: false };
  }

  endUndoStep() {
    const held = this.heldStep;
    if (!held) return;
    this.heldStep = undefined;
    if (held.changed) this.recorder.step();
  }

  constructor(public registry: NodeRegistry) {
    super();
  }

  /** Takes the node of a dropped wasm file wherever else graphs are built, before it is placed */
  onregister?: (node: NodeDefinition, wasm: ArrayBuffer) => Promise<void> | void;

  /** The node of a wasm file dropped onto the editor */
  async registerNode(wasm: ArrayBuffer) {
    const node = await this.registry.register('', wasm);
    await this.onregister?.(node, wasm);
    return node;
  }

  /** The editor only needs the definitions, the runtime loads the code of the nodes it runs */
  loadDefinitions(nodeIds: NodeId[]) {
    return this.registry.loadDefinitions?.(nodeIds) ?? this.registry.load(nodeIds);
  }

  /** The open level from the live nodes, every other level from where it is stored */
  private levelState(groupId: number | null): Level {
    if (groupId !== this.currentGroupId) return this.storedLevel(groupId);
    return { nodes: [...this.nodes.values()], edges: this.edges.map(e => serializeEdge(e)) };
  }

  private storedLevel(groupId: number | null): Level {
    if (groupId === null) return this.root;
    const group = this.getGroup(groupId);
    return { nodes: group?.nodes ?? [], edges: group?.edges ?? [] };
  }

  /** Writes the open level to where it is stored */
  private storeLevel() {
    const level = {
      nodes: [...this.nodes.values()].map(n => serializeNode(n)),
      edges: this.edges.map(e => serializeEdge(e))
    };
    if (this.currentGroupId === null) {
      this.root = level;
      return;
    }
    const group = this.getGroup(this.currentGroupId);
    if (!group) return;
    group.nodes = level.nodes;
    group.edges = level.edges;
  }

  // Recomputed only after the graph changed: the history, the build, the database and the
  // panels share one walk over the reactive state per change
  private serialized = $derived.by(() => this.serializeState());

  /** A copy of the graph, the caller may change it */
  serialize(): Graph {
    return structuredClone(this.serialized);
  }

  private serializeState(): Graph {
    const root = this.levelState(null);
    const nodes = root.nodes.map(n => serializeNode(n));
    const edges = root.edges.map(e => serializeEdge(e));

    const groups = this.groups?.map((group) => {
      const level = this.levelState(group.id);
      const serialized = { id: group.id } as GroupDefinition;
      if (group.name !== undefined) serialized.name = group.name;
      if (group.inputs !== undefined) serialized.inputs = toPlain(group.inputs);
      if (group.outputs !== undefined) serialized.outputs = toPlain(group.outputs);
      serialized.nodes = level.nodes.map(n => serializeNode(n));
      serialized.edges = level.edges.map(e => serializeEdge(e));
      return serialized;
    });

    const serialized = {} as Graph;
    if (this.id !== undefined) serialized.id = this.id;
    serialized.version = CURRENT_GRAPH_VERSION;
    serialized.settings = toPlain(this.settings);
    if (this.meta !== undefined) serialized.meta = toPlain(this.meta);
    if (groups !== undefined) serialized.groups = groups;
    serialized.nodes = nodes;
    serialized.edges = edges;
    log.log('serializing graph', serialized);
    return serialized;
  }

  /** the collections the add menu offers, like `max/plantarium` */
  private collections: ReadonlySet<string> = new SvelteSet<string>();

  /** A value changed in the settings panel */
  setSetting(key: string, value: unknown) {
    if (this.status !== 'idle' || !(key in this.settingTypes)) return;
    if (JSON.stringify(this.settings[key]) === JSON.stringify(value)) return;
    this.settings[key] = value;
    this.commit({ coalesce: true });
  }

  private usedNodeTypes() {
    const types = new SvelteSet<NodeId>();
    for (const groupId of [null, ...this.groups.map((g) => g.id)]) {
      for (const node of this.levelState(groupId).nodes) types.add(node.type);
    }
    return types;
  }

  /**
   * Only the nodes in the graph declare its settings, not every node of their collections or
   * every node the registry loaded for earlier graphs. Adding or removing a node can change them.
   */
  private updateSettings(values = this.settings) {
    const definitions = [...this.usedNodeTypes()].flatMap((type) =>
      this.registry.getNode(type) ?? []
    );
    const { types, values: filled } = collectSettings(definitions, values);
    // most changes of the graph leave its settings as they are, the panel keeps its inputs then
    if (values === this.settings && sameKeys(types, this.settingTypes)) return;
    this.settingTypes = types;
    this.settings = filled;
  }

  /**
   * The definitions of every collection the registry lists, and of the collections the graph
   * uses. The registry keeps every node it ever loaded, a dropped node of an earlier graph is
   * left out.
   */
  getNodeDefinitions() {
    const all = this.registry.getAllNodes();
    if (this.collections.size === 0) return all;
    return all.filter((d) => {
      const [user, collection] = d.id.split('/');
      return user === '__internal' || this.collections.has(`${user}/${collection}`);
    });
  }

  getLinkedNodes(node: NodeInstance) {
    const nodes = new SvelteSet<NodeInstance>();
    const stack = [node];
    while (stack.length) {
      const n = stack.pop();
      if (!n) continue;
      nodes.add(n);
      const children = this.getChildren(n);
      const parents = this.getParentsOfNode(n);
      const newNodes = [...children, ...parents].filter((n) => !nodes.has(n));
      stack.push(...newNodes);
    }
    return [...nodes.values()];
  }

  getEdgeId(e: Edge) {
    return `${e[0].id}-${e[1]}-${e[2].id}-${e[3]}`;
  }

  getEdgeById(id: string): Edge | undefined {
    return this.edges.find((e) => this.getEdgeId(e) === id);
  }

  // Wires the node in between the two nodes of the edge and moves them apart where the node
  // has no room. Returns the previous x of every node that was moved aside.
  dropNodeOnEdge(nodeId: number, edge: Edge): Map<NodeInstance, number> {
    const draggedNode = this.getNode(nodeId);
    if (!draggedNode) return new SvelteMap();

    const [fromNode, fromSocketIdx, toNode, toSocketKey] = edge;

    const dragged = this.getNodeSockets(draggedNode);
    const draggedInputs = Object.entries(dragged.inputs);
    const draggedOutputs = dragged.outputs;

    const edgeOutputSocketType = this.getSocketType(fromNode, fromSocketIdx);
    const targetInput = this.getNodeSockets(toNode).inputs[toSocketKey];

    const bestInputEntry = draggedInputs.find(([, input]) =>
      isInputConnectable(input) && inputAccepts(input, edgeOutputSocketType)
    );

    const bestOutputIdx = draggedOutputs.findIndex(outputType =>
      inputAccepts(targetInput, outputType)
    );

    if (!bestInputEntry || bestOutputIdx === -1) {
      log.error('Could not find compatible sockets for drop');
      return new SvelteMap();
    }

    return this.transaction(() => {
      this.removeEdge(edge);
      this.createEdge(fromNode, fromSocketIdx, draggedNode, bestInputEntry[0]);
      this.createEdge(draggedNode, bestOutputIdx, toNode, toSocketKey);
      return this.makeRoomForNode(draggedNode, fromNode, toNode);
    });
  }

  // Pushes `to` and everything downstream of it right, and `from` and everything upstream of it
  // left, until `node` is a column gap away from both. Only the x positions change, the rest of
  // the layout is left alone. Returns the previous x of every moved node.
  private makeRoomForNode(node: NodeInstance, from: NodeInstance, to: NodeInstance) {
    const minDistance = NODE_WIDTH + COLUMN_GAP;
    const moved = new SvelteMap<NodeInstance, number>();

    const shift = (nodes: Set<NodeInstance>, dx: number) => {
      for (const n of nodes) {
        if (!moved.has(n)) moved.set(n, n.position[0]);
        n.position[0] += dx;
      }
    };

    const missingRight = node.position[0] + minDistance - to.position[0];
    if (missingRight > 0) shift(new SvelteSet([to, ...this.getChildren(to)]), missingRight);

    const missingLeft = from.position[0] + minDistance - node.position[0];
    if (missingLeft > 0) shift(new SvelteSet([from, ...this.getParentsOfNode(from)]), -missingLeft);

    return moved;
  }

  getPossibleDropOnEdges(nodeId: number): Edge[] {
    const draggedNode = this.getNode(nodeId);
    if (!draggedNode) return [];

    const dragged = this.getNodeSockets(draggedNode);
    const draggedInputs = Object.values(dragged.inputs);
    const draggedOutputs = dragged.outputs;

    const parentIds = new SvelteSet(this.getParentsOfNode(draggedNode).map(n => n.id));

    return this.edges.filter((edge) => {
      const [fromNode, fromSocketIdx, toNode, toSocketKey] = edge;

      if (parentIds.has(toNode.id)) return false;

      if (fromNode.id === nodeId || toNode.id === nodeId) return false;

      const edgeOutputSocketType = this.getSocketType(fromNode, fromSocketIdx);
      const canPlugIntoDragged = draggedInputs.some(input =>
        isInputConnectable(input) && inputAccepts(input, edgeOutputSocketType)
      );

      if (!canPlugIntoDragged) return false;

      const targetInput = this.getNodeSockets(toNode).inputs[toSocketKey];

      const draggedCanPlugIntoTarget = draggedOutputs.some(outputType =>
        inputAccepts(targetInput, outputType)
      );

      return draggedCanPlugIntoTarget;
    });
  }

  tryConnectToDebugNode(nodeId: number) {
    const node = this.nodes.get(nodeId);
    if (!node) return;
    if (isDebugNode(node.type)) return;
    const outputCount = this.getNodeSockets(node).outputs.length;
    if (!outputCount) return;
    return this.transaction(() => {
      const debugNode = this.nodes.values().find(n => isDebugNode(n.type))
        ?? this.createNode({ type: DEBUG_NODE, position: [0, 0], props: {} });
      if (!debugNode) return;

      // above and to the right of the debugged node, nodes are 20 units wide
      const debugType = this.getNodeType(debugNode);
      const height = debugType ? getNodeHeight(debugType) : 5;
      debugNode.position = [node.position[0] + 25, node.position[1] - height - 5];

      // debugging a node again steps through its outputs
      const connected = this.getEdgesToNode(debugNode).find(e => e[0].id === node.id);
      const output = connected ? (connected[1] + 1) % outputCount : 0;
      if (connected && connected[1] === output) {
        this.commit({ build: false });
      } else {
        this.createEdge(node, output, debugNode, 'input');
      }

      return debugNode;
    });
  }

  /** A node added from the end of an edge dragged out of `socket`, connected to it where the types fit */
  createNodeFromSocket(
    node: Pick<NodeInstance, 'type' | 'position' | 'props'>,
    socket: Socket | null
  ) {
    return this.transaction(() => {
      const newNode = this.createNode({
        type: node.type,
        position: node.position,
        props: node.props
      });
      if (!newNode || !socket) return newNode;

      const newSockets = this.getNodeSockets(newNode);
      if (typeof socket.index === 'number') {
        const socketType = this.getSocketType(socket.node, socket.index);
        const input = Object.entries(newSockets.inputs).find(([, input]) =>
          inputAccepts(input, socketType)
        );
        if (input) this.createEdge(socket.node, socket.index, newNode, input[0]);
      } else {
        const input = this.getNodeSockets(socket.node).inputs[socket.index];
        const outputIndex = newSockets.outputs.findIndex((output) => inputAccepts(input, output));
        if (outputIndex !== -1) this.createEdge(newNode, outputIndex, socket.node, socket.index);
      }
      return newNode;
    });
  }

  getEdgesBetweenNodes(nodes: NodeInstance[]): SerializedEdge[] {
    const ids = new SvelteSet(nodes.map((n) => n.id));
    return this.edges
      .filter((e) => ids.has(e[0].id) && ids.has(e[2].id))
      .map((e) => serializeEdge(e));
  }

  /** Makes the nodes and edges of `level` the open ones, as new instances */
  private openLevel(level: Level) {
    this.nodes.clear();
    for (const node of level.nodes) this.addNode(node);
    this.edges = this.liveEdges(level.edges);
  }

  /** A new instance of the node in the open level */
  private addNode(node: SerializedNode) {
    const instance: NodeInstance = $state({
      ...serializeNode(node),
      position: positionOf(node),
      state: { type: this.registry.getNode(node.type) }
    });
    this.nodes.set(instance.id, instance);
    return instance;
  }

  /** The edges between the nodes of the open level */
  private liveEdges(edges: SerializedEdge[]): Edge[] {
    return edges.flatMap((edge) => {
      const from = this.nodes.get(edge[0]);
      const to = this.nodes.get(edge[2]);
      if (!from || !to) {
        log.warn('Dropping orphaned edge', edge);
        return [];
      }
      return [[from, edge[1], to, edge[3]] as Edge];
    });
  }

  // Recomputed only after an edge was added or removed, moving a node or changing a value keeps it
  private relations = $derived(buildRelations(this.edges));

  /** The nodes behind the outputs of `node`, each once */
  childrenOf(node: NodeInstance): NodeInstance[] {
    return this.relations.children.get(node.id) ?? [];
  }

  /** The nodes in front of the inputs of `node`, each once */
  parentsOf(node: NodeInstance): NodeInstance[] {
    return this.relations.parents.get(node.id) ?? [];
  }

  async load(graph: Graph) {
    const a = performance.now();
    this.flush();
    this.buildPending = true;

    try {
      graph = migrateGraph(graph);
    } catch (error) {
      log.error('failed to migrate graph', error);
      toast(
        error instanceof GraphVersionError
          ? `This graph was saved by a newer version of Nodarium (graph version ${error.graphVersion}, `
            + `this app knows up to ${CURRENT_GRAPH_VERSION}). Reload the page to update the app.`
          : error instanceof Error
          ? error.message
          : String(error),
        'error'
      );
      this.status = 'error';
      this.buildPending = false;
      return;
    }

    this.settingTypes = {};
    this.settings = {};
    const groups = $state.snapshot(graph.groups ?? []);
    this.meta = graph.meta;
    this.groups = groups;
    this.status = 'loading';
    this.id = graph.id;

    log.info(
      'loading graph',
      { nodes: graph.nodes, edges: graph.edges, id: graph.id }
    );

    const nodeIds = Array
      .from(
        new SvelteSet(
          [
            ...graph.nodes,
            ...groups.flatMap(g => g.nodes)
          ]
            .filter(n => n && 'type' in n)
            .map((n) => n.type)
        )
      );

    try {
      await this.loadDefinitions(nodeIds);
    } catch (error) {
      log.error('failed to load the node types of the graph', error);
      toast(
        `Could not load the graph, a node type it uses is not available (any more): ${
          error instanceof Error ? error.message : String(error)
        }`,
        'error'
      );
      this.status = 'error';
      this.buildPending = false;
      return;
    }

    // a graph written by hand may leave out the positions of its nodes
    const placed = placeNodes(graph, {
      height: (node) => {
        const type = this.registry.getNode(node.type);
        return type ? getNodeHeight(type) : 20;
      }
    });
    const wasPlaced = placed !== graph;
    if (wasPlaced) {
      graph = placed;
      this.groups = $state.snapshot(placed.groups);
    }

    const allCollections = new SvelteSet<`${string}/${string}`>(this.registry.baseCollections);
    for (const id of nodeIds) {
      const [user, collection] = id.split('/');
      if (user === '__internal') continue;
      allCollections.add(`${user}/${collection}`);
    }
    this.collections = allCollections;
    const loadCollection = (collection: `${string}/${string}`) => {
      this.registry.loadCollection?.(collection)
        .catch((error) => log.error(`failed to load the collection ${collection}`, error));
    };
    for (const collection of allCollections) loadCollection(collection);
    this.registry.listCollections?.()
      .then((listed) => {
        for (const collection of listed) {
          if (allCollections.has(collection)) continue;
          allCollections.add(collection);
          loadCollection(collection);
        }
      })
      .catch((error) => log.error('failed to list the collections', error));

    log.info('loaded node types', this.registry.getAllNodes());

    this.path = [];

    this.heldStep = undefined;
    this.recorder.reset();
    this.openLevel(graph);
    this.updateSettings(graph.settings);
    this.recorder.step();
    this.status = 'idle';
    if (wasPlaced) this.emit('placed', graph);
    log.log(`Graph loaded in ${performance.now() - a}ms`);
    setTimeout(() => this.execute(), 100);
  }

  getNode(id: number) {
    return this.nodes.get(id);
  }

  private currentGroup = $derived(
    this.currentGroupId === null ? undefined : this.getGroup(this.currentGroupId)
  );

  // Recomputed only after a group was added, removed or renamed or the path changed
  private groupOptions = $derived(
    groupOptions(this.groups, new SvelteSet(this.path.map((entry) => entry.groupId)))
  );

  getNodeType(node: NodeInstance) {
    switch (node.type) {
      case GROUP_INPUT:
        return this.currentGroup ? groupInputType(this.currentGroup) : node.state.type;
      case GROUP_OUTPUT:
        return this.currentGroup ? groupOutputType(this.currentGroup) : node.state.type;
      case GROUP_INSTANCE:
        return this.groupInstanceType(node);
      default:
        return exposeInputs(node.state.type, node.exposed);
    }
  }

  private groupInstanceType(node: NodeInstance) {
    const groupId = node.props?.groupId as number;
    const group = groupId ? this.getGroup(groupId) : undefined;
    if (groupId && !group) {
      log.error(`Group not found: ${groupId}`);
      return;
    }
    return groupInstanceType(this.registry.getNode(node.type), group, this.groupOptions);
  }

  // Shows or hides one of the hidden inputs of a node on the node itself.
  // Edges into an input that gets hidden again are removed.
  setInputExposed(node: NodeInstance, key: string, exposed: boolean) {
    const current = node.exposed ?? [];
    if (current.includes(key) === exposed) return;

    this.transaction(() => {
      if (exposed) {
        node.exposed = [...current, key];
      } else {
        node.exposed = current.filter(k => k !== key);
        for (const edge of this.getEdgesToNode(node)) {
          if (edge[3] === key) this.removeEdge(edge);
        }
      }
      this.commit();
    });
  }

  // The sockets edges can connect to. The Group Input node exposes its output
  // sockets as inputs, see getNodeType().
  getNodeSockets(node: NodeInstance): {
    inputs: Record<string, NodeInput>;
    outputs: string[];
  } {
    const nodeType = this.getNodeType(node);
    if (node.type === GROUP_INPUT) {
      return { inputs: {}, outputs: Object.values(nodeType?.inputs ?? {}).map(i => i.type) };
    }
    return {
      inputs: nodeType?.inputs ?? {},
      outputs: (nodeType?.outputs ?? []).map(o => getOutputType(o)!)
    };
  }

  getSocketType(node: NodeInstance, index: string | number): string | undefined {
    const { inputs, outputs } = this.getNodeSockets(node);
    return typeof index === 'string' ? inputs[index]?.type : outputs[index];
  }

  /** Everything downstream of `node`, each node once */
  getChildren(node: NodeInstance): NodeInstance[] {
    return reachable(node, this.relations.children);
  }

  getNodesBetween(from: NodeInstance, to: NodeInstance): NodeInstance[] | undefined {
    //  < - - - - from
    const toParents = this.getParentsOfNode(to);
    //  < - - - - from - - - - to
    const fromParents = this.getParentsOfNode(from);
    if (toParents.includes(from)) {
      const fromChildren = this.getChildren(from);
      return toParents.filter((n) => fromChildren.includes(n));
    } else if (fromParents.includes(to)) {
      const toChildren = this.getChildren(to);
      return fromParents.filter((n) => toChildren.includes(n));
    } else {
      // these two nodes are not connected
      return;
    }
  }

  removeNode(node: NodeInstance, { restoreEdges = false } = {}) {
    log.log('removing node', { id: node.id, type: node.type, restoreEdges });
    const edgesToNode = this.edges.filter((edge) => edge[2].id === node.id);
    const edgesFromNode = this.edges.filter((edge) => edge[0].id === node.id);
    this.transaction(() => {
      for (const edge of [...edgesToNode, ...edgesFromNode]) {
        this.removeEdge(edge);
      }

      if (restoreEdges) {
        const outputSockets = edgesToNode.map((e) => [e[0], e[1]] as const);
        const inputSockets = edgesFromNode.map((e) => [e[2], e[3]] as const);

        for (const [to, toSocket] of inputSockets) {
          for (const [from, fromSocket] of outputSockets) {
            const outputType = this.getSocketType(from, fromSocket);
            const inputType = this.getSocketType(to, toSocket);
            if (outputType === inputType) this.createEdge(from, fromSocket, to, toSocket);
          }
        }
      }

      this.nodes.delete(node.id);
      this.commit();
    });
  }

  // arranges the nodes of the current level in columns, see layoutGraph
  formatNodes() {
    const positions = layoutGraph(
      [...this.nodes.values()],
      this.edges.map((e) => serializeEdge(e)),
      (node) => this.getNodeType(node)
    );
    for (const node of this.nodes.values()) {
      const position = positions.get(node.id);
      if (position) node.position = position;
    }
    this.commit({ build: false });
  }

  smartConnect(from: NodeInstance, to: NodeInstance): Edge | undefined {
    const inputs = Object.entries(this.getNodeSockets(to).inputs);
    const outputs = this.getNodeSockets(from).outputs;
    for (let i = 0; i < inputs.length; i++) {
      const [inputName, input] = inputs[i];
      for (let o = 0; o < outputs.length; o++) {
        const output = outputs[o];
        if (input.type === output) {
          return this.createEdge(from, o, to, inputName);
        }
      }
    }
  }

  getGroup(id: number) {
    return this.groups.find(g => g.id === id);
  }

  renameGroup(groupId: number, name: string) {
    log.log('renaming group', { groupId, name });
    const group = this.getGroup(groupId);
    if (!group) return;
    group.name = name;
    this.commit({ build: false });
  }

  removeGroupInput(groupId: number, key: string) {
    const group = this.getGroup(groupId);
    if (!group?.inputs || !(key in group.inputs)) return;
    log.log('removing group input', { groupId, key });
    this.transaction(() => this.removeGroupInputOf(group, key));
  }

  private removeGroupInputOf(group: GroupDefinition, key: string) {
    const groupId = group.id;
    // Group Input sockets are positional, so the sockets after the removed one move up by one
    const index = Object.keys(group.inputs!).indexOf(key);
    const inputs = { ...group.inputs };
    delete inputs[key];
    group.inputs = inputs;

    if (this.currentGroupId === groupId) {
      for (const edge of this.edges.filter(e => e[0].type === GROUP_INPUT)) {
        if (edge[1] === index) this.removeEdge(edge);
        else if (edge[1] > index) edge[1]--;
      }
    } else {
      const inputNodeId = group.nodes.find(n => n.type === GROUP_INPUT)?.id;
      group.edges = group.edges
        .filter(e => !(e[0] === inputNodeId && e[1] === index))
        .map(e =>
          e[0] === inputNodeId && e[1] > index ? [e[0], e[1] - 1, e[2], e[3]] as SerializedEdge : e
        );
    }

    // Drop the edges into the removed socket on every instance of the group
    for (const edge of this.edges.filter(e => groupIdOf(e[2]) === groupId && e[3] === key)) {
      this.removeEdge(edge);
    }
    const withoutInput = (level: Level) =>
      level.edges.filter(e =>
        !(e[3] === key && groupIdOf(level.nodes.find(n => n.id === e[2])) === groupId)
      );
    if (this.isInsideGroup) this.root = { ...this.root, edges: withoutInput(this.root) };
    for (const level of this.groups) {
      if (level.id !== this.currentGroupId) level.edges = withoutInput(level);
    }

    this.commit();
  }

  enterGroup(nodeId: number): boolean {
    const groupNode = this.getNode(nodeId);
    if (!groupNode || groupNode.type !== GROUP_INSTANCE) return false;

    log.log('entering group', { nodeId });

    const groupId = groupNode.props?.groupId as number;
    const group = this.getGroup(groupId);
    if (!group) return false;

    this.storeLevel();
    this.path.push({ groupId, nodeId });
    log.log('entered group', { groupId, depth: this.path.length });
    this.openLevel(group);
    return true;
  }

  exitGroup() {
    log.log('exiting group', { depth: this.path.length });
    const entry = this.path.at(-1);
    if (!entry) return;

    this.storeLevel();
    this.path.pop();
    this.openLevel(this.storedLevel(this.currentGroupId));
    this.commit();

    return { nodeId: entry.nodeId };
  }

  // Returns a function that hands out unused IDs. IDs are kept unique across all levels and the
  // group IDs, so a node can't be mistaken for one of another level.
  private createIdAllocator() {
    const usedIds = new SvelteSet<number>([
      ...this.nodes.keys(),
      ...(this.isInsideGroup ? this.root.nodes.map(n => n.id) : []),
      ...this.groups.map(g => g.id),
      ...this.groups.flatMap(g => g.id === this.currentGroupId ? [] : g.nodes.map(n => n.id))
    ]);
    return () => {
      let id = 0;
      while (usedIds.has(id)) id++;
      usedIds.add(id);
      return id;
    };
  }

  createGraph(nodes: SerializedNode[], edges: [number, number, number, string][]) {
    // A level has a single Group Input and Output node, pasting must not add another
    // one (only the first one is used). Their edges are dropped with them.
    nodes = nodes.filter(n => n.type !== GROUP_INPUT && n.type !== GROUP_OUTPUT);

    const idMap = new SvelteMap<number, number>();
    const nextId = this.createIdAllocator();

    const instances: NodeInstance[] = nodes.map((node) => {
      const type = this.registry.getNode(node.type);
      if (!type && !node.type.startsWith(INTERNAL_PREFIX)) {
        throw new Error(`Node type not found: ${node.type}`);
      }
      const instance: NodeInstance = $state({
        ...node,
        id: nextId(),
        position: positionOf(node),
        state: { type }
      });
      idMap.set(node.id, instance.id);
      return instance;
    });

    for (const node of instances) {
      this.nodes.set(node.id, node);
    }

    for (const [fromId, fromSocket, toId, toSocket] of edges) {
      const from = instances.find((n) => n.id === idMap.get(fromId));
      const to = instances.find((n) => n.id === idMap.get(toId));
      if (from && to) this.edges.push([from, fromSocket, to, toSocket]);
    }

    this.commit({ build: false });
    return instances;
  }

  getUnusedGroups() {
    const usedGroupIds = new SvelteSet<number>();
    const queue = [...this.levelState(null).nodes];
    while (queue.length) {
      const groupId = groupIdOf(queue.pop());
      if (groupId === undefined || usedGroupIds.has(groupId)) continue;
      usedGroupIds.add(groupId);
      queue.push(...this.levelState(groupId).nodes);
    }
    return this.groups.filter(g => !usedGroupIds.has(g.id));
  }

  removeUnusedGroups() {
    const unused = this.getUnusedGroups();
    const unusedIds = new SvelteSet(unused.map(g => g.id));
    this.groups = this.groups.filter(g => !unusedIds.has(g.id));
    this.commit({ build: false });
    return unused.length;
  }

  groupNodes(nodeIds: number[]) {
    const grouped = groupSelection(this.levelState(this.currentGroupId), new SvelteSet(nodeIds), {
      outputType: (nodeId, output) => this.getSocketType(this.nodes.get(nodeId)!, output),
      input: (nodeId, key) => this.getNodeType(this.nodes.get(nodeId)!)?.inputs?.[key],
      nextId: this.createIdAllocator()
    });
    if (!grouped) return;
    const { group, instance, edges } = grouped;
    log.log('grouping nodes', { group });

    return this.transaction(() => {
      this.groups.push(group);
      for (const node of group.nodes) this.nodes.delete(node.id);
      const groupNode = this.addNode(instance);
      this.edges = this.liveEdges(edges);
      this.commit();
      return groupNode;
    });
  }

  ungroupNode(nodeId: number) {
    const groupNode = this.getNode(nodeId);
    if (!groupNode || groupNode.type !== GROUP_INSTANCE) return false;

    const groupId = groupNode.props?.groupId as number;
    const group = this.getGroup(groupId);
    if (!group) return false;

    log.log('ungrouping node', { groupId, group });
    const { nodes, edges } = ungroupInstance(
      this.levelState(this.currentGroupId),
      groupNode,
      group,
      this.createIdAllocator()
    );

    return this.transaction(() => {
      const instances = nodes.map((node) => this.addNode(node));
      // createEdge() checks the socket types, the types of a group can be changed after it was wired
      for (const [fromId, fromSocket, toId, toSocket] of edges) {
        this.createEdge(this.nodes.get(fromId)!, fromSocket, this.nodes.get(toId)!, toSocket);
      }
      this.removeNode(groupNode);
      return instances;
    });
  }

  createNode({
    type,
    position,
    props = {}
  }: {
    type: NodeInstance['type'];
    position: NodeInstance['position'];
    props: NodeInstance['props'];
  }) {
    const nodeType = this.registry.getNode(type);
    if (!nodeType && !type.startsWith(INTERNAL_PREFIX)) {
      log.error(`Node type not found: ${type}`);
      return;
    }

    // a group node from the add menu comes without a group
    if (type === GROUP_INSTANCE && props?.groupId === undefined) {
      const group = this.groupOptions[0];
      if (group) props = { ...props, groupId: group.value };
    }

    const node: NodeInstance = $state({
      id: this.createIdAllocator()(),
      type,
      position,
      state: { type: nodeType },
      props
    });

    log.log('creating node', { id: node.id, type, position, props });
    this.nodes.set(node.id, node);

    this.commit({ build: false });

    return node;
  }

  createEdge(
    from: NodeInstance,
    fromSocket: number,
    to: NodeInstance,
    toSocket: string
  ): Edge | undefined {
    const existingEdges = this.getEdgesToNode(to);

    const existingEdge = existingEdges.find(
      (e) => e[0].id === from.id && e[1] === fromSocket && e[3] === toSocket
    );
    if (existingEdge) {
      log.error('Edge already exists', existingEdge);
      return;
    }

    const fromSocketType = this.getSocketType(from, fromSocket);
    const toInput = this.getNodeSockets(to).inputs[toSocket];

    if (toInput?.internal) {
      log.error(`Input ${toSocket} of ${to.type} is internal and can not be connected`);
      return;
    }

    if (!toInput || !inputAccepts(toInput, fromSocketType)) {
      log.error(`Socket types do not match: ${fromSocketType} !== ${toInput?.type}`);
      return;
    }

    const edgeToBeReplaced = this.edges.find(
      (e) => e[2].id === to.id && e[3] === toSocket
    );
    const edge = [from, fromSocket, to, toSocket] as Edge;

    log.log('creating edge', { from: from.id, fromSocket, to: to.id, toSocket });
    return this.transaction(() => {
      if (edgeToBeReplaced) this.removeEdge(edgeToBeReplaced);
      this.edges.push(edge);
      this.commit();
      return edge;
    });
  }

  /**
   * The input a muted node passes through to its outputs: the first input that
   * can be wired. A group instance skips its internal group selector.
   */
  getMutePassthroughInput(node: NodeInstance): string | undefined {
    const inputs = this.getNodeType(node)?.inputs;
    if (!inputs) return;
    const isGroup = node.type === GROUP_INSTANCE;
    const [key, input] = Object.entries(inputs).find(([k]) => !(isGroup && k === 'groupId')) ?? [];
    if (!key || !input) return;
    // setting, seed and creator inputs can't be wired, so nothing could pass through
    if (input.type === 'seed' || input.type === 'creator' || input.setting) return;
    return key;
  }

  muteNodes(nodeIds: number[]) {
    this.transaction(() => nodeIds.forEach(id => this.muteNode(id)));
  }

  muteNode(activeNodeId: number): void {
    const node = this.getNode(activeNodeId);
    if (!node) return;
    if (!this.getMutePassthroughInput(node)) return;
    if (node.type === GROUP_OUTPUT) return;
    if (node.type === GROUP_INPUT) return;
    if (isDebugNode(node.type)) return;
    if (isOutputNode(node.type)) return;
    node.muted = !node.muted;
    this.commit();
  }

  // History entries are full graphs (see serialize()) and one history spans all levels. If a
  // group on the path doesn't exist in the restored graph (e.g. undoing the grouping) we end up
  // at the deepest level that still does.
  private restoreState(state: Graph) {
    this.groups = state.groups ?? [];
    this.root = { nodes: state.nodes, edges: state.edges };
    this.path = this.pathStillIn(state);
    this.openLevel(this.storedLevel(this.currentGroupId));
    this.updateSettings(state.settings);
  }

  /** The longest start of the path whose group instances and groups exist in `state` */
  private pathStillIn(state: Graph) {
    let level: Level = state;
    const path: typeof this.path = [];
    for (const entry of this.path) {
      const group = this.getGroup(entry.groupId);
      const instance = level.nodes.find(n => n.id === entry.nodeId);
      if (!group || instance?.props?.groupId !== group.id) break;
      path.push({ ...entry });
      level = group;
    }
    return path;
  }

  undo() {
    log.log('undo');
    this.endUndoStep();
    this.restore(this.recorder.undo());
  }

  redo() {
    log.log('redo');
    this.endUndoStep();
    this.restore(this.recorder.redo());
  }

  private restore(graph: Graph | undefined) {
    if (!graph) return;
    this.restoreState(graph);
    this.execute();
    this.emit('save', this.serialize());
  }

  /** Adds the pending undo step and saves the graph right away, e.g. before the page is closed */
  flush() {
    this.endUndoStep();
    this.recorder.flush();
  }

  /** Everything upstream of `node`, each node once */
  getParentsOfNode(node: NodeInstance): NodeInstance[] {
    return reachable(node, this.relations.parents);
  }

  getPossibleNodes(socket: Socket): NodeDefinition[] {
    const allDefinitions = this.getNodeDefinitions();

    // dragging from an input, look for definitions with a matching output
    if (typeof socket.index === 'string') {
      const input = this.getNodeSockets(socket.node).inputs[socket.index];
      if (!isInputConnectable(input)) return [];
      return allDefinitions.filter(d =>
        d.outputs?.some(output => inputAccepts(input, getOutputType(output)))
      );
    }

    // dragging from an output, look for definitions with a matching input
    const outputType = this.getSocketType(socket.node, socket.index);
    if (!outputType) return [];
    return allDefinitions.filter(d =>
      Object.values(d.inputs ?? {}).some(input =>
        isInputConnectable(input) && inputAccepts(input, outputType)
      )
    );
  }

  getPossibleSockets({ node, index }: Socket): [NodeInstance, string | number][] {
    const ownType = this.getSocketType(node, index);
    if (!ownType) return [];

    const sockets: [NodeInstance, string | number][] = [];

    if (typeof index === 'string') {
      const input = this.getNodeSockets(node).inputs[index];
      if (!isInputConnectable(input)) return [];
      const children = new SvelteSet(this.getChildren(node).map((n) => n.id));
      const nodes = this.nodeArray.filter(
        (n) => n.id !== node.id && !children.has(n.id)
      );

      for (const node of nodes) {
        const outputs = this.getNodeSockets(node).outputs;
        for (let index = 0; index < outputs.length; index++) {
          if (inputAccepts(input, outputs[index])) {
            sockets.push([node, index]);
          }
        }
      }
    } else if (typeof index === 'number') {
      const parents = new SvelteSet(this.getParentsOfNode(node).map((n) => n.id));
      const nodes = this.nodeArray.filter(
        (n) => n.id !== node.id && !parents.has(n.id)
      );

      const edges = new SvelteMap<number, string[]>();
      this.getEdgesFromNode(node)
        .filter((e) => e[1] === index)
        .forEach((e) => {
          if (edges.has(e[2].id)) {
            edges.get(e[2].id)?.push(e[3]);
          } else {
            edges.set(e[2].id, [e[3]]);
          }
        });

      for (const node of nodes) {
        const inputs = this.getNodeSockets(node).inputs;
        for (const key in inputs) {
          if (!isInputConnectable(inputs[key])) continue;

          if (inputAccepts(inputs[key], ownType) && !edges.get(node.id)?.includes(key)) {
            sockets.push([node, key]);
          }
        }
      }
    }

    return sockets;
  }

  removeEdge(edge: Edge) {
    log.log('removing edge', {
      from: edge[0].id,
      fromSocket: edge[1],
      to: edge[2].id,
      toSocket: edge[3]
    });
    const id0 = edge[0].id;
    const sid0 = edge[1];
    const id2 = edge[2].id;
    const sid2 = edge[3];

    const _edge = this.edges.find(
      (e) => e[0].id === id0 && e[1] === sid0 && e[2].id === id2 && e[3] === sid2
    );

    if (!_edge) return;

    this.edges = this.edges.filter((e) => !areEdgesEqual(e, edge));
    this.commit();
  }

  getEdgesToNode(node: NodeInstance) {
    return this.edges.filter((e) => e[2].id === node.id);
  }

  getEdgesFromNode(node: NodeInstance) {
    return this.edges.filter((e) => e[0].id === node.id);
  }
}
