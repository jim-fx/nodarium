import throttle from '$lib/helpers/throttle';
import { RemoteNodeRegistry } from '$lib/node-registry/index';
import type {
  Edge,
  Graph,
  NodeDefinition,
  NodeId,
  NodeInput,
  NodeInstance,
  NodeRegistry,
  Socket
} from '@nodarium/types';
import { fastHashString } from '@nodarium/utils';
import { createLogger } from '@nodarium/utils';
import { SvelteMap, SvelteSet } from 'svelte/reactivity';
import EventEmitter from './helpers/EventEmitter';
import { HistoryManager } from './history-manager';

const logger = createLogger('graph-manager');
logger.mute();

const remoteRegistry = new RemoteNodeRegistry('');

const clone = 'structuredClone' in self
  ? self.structuredClone
  : (args: unknown) => JSON.parse(JSON.stringify(args));

export function areSocketsCompatible(
  output: string | undefined,
  inputs: string | (string | undefined)[] | undefined
) {
  if (output === '*') return true;
  if (Array.isArray(inputs) && output) {
    return inputs.includes('*') || inputs.includes(output);
  }
  return inputs === output || inputs === '*';
}

function areEdgesEqual(firstEdge: Edge, secondEdge: Edge) {
  if (firstEdge[0].id !== secondEdge[0].id) {
    return false;
  }

  if (firstEdge[1] !== secondEdge[1]) {
    return false;
  }

  if (firstEdge[2].id !== secondEdge[2].id) {
    return false;
  }

  if (firstEdge[3] !== secondEdge[3]) {
    return false;
  }

  return true;
}

export class GraphManager extends EventEmitter<{
  save: Graph;
  result: unknown;
  settings: {
    types: Record<string, NodeInput>;
    values: Record<string, unknown>;
  };
}> {
  status = $state<'loading' | 'idle' | 'error'>();
  loaded = false;

  graph: Graph = { id: 0, nodes: [], edges: [] };
  id = $state(0);

  nodes = new SvelteMap<number, NodeInstance>();

  edges = $state<Edge[]>([]);

  settingTypes: Record<string, NodeInput> = {};
  settings = $state<Record<string, unknown>>();

  currentUndoGroup: number | null = null;

  inputSockets = $derived.by(() => {
    const s = new SvelteSet<string>();
    for (const edge of this.edges) {
      s.add(`${edge[2].id}-${edge[3]}`);
    }
    return s;
  });

  history: HistoryManager = new HistoryManager();
  execute = throttle(() => {
    if (this.loaded === false) return;
    this.emit('result', this.serialize());
  }, 10);

  constructor(public registry: NodeRegistry) {
    super();
  }

  serialize(): Graph {
    const nodes = Array.from(this.nodes.values()).map((node) => ({
      id: node.id,
      position: [...node.position],
      type: node.type,
      props: node.props
    })) as NodeInstance[];
    const edges = this.edges.map((edge) => [
      edge[0].id,
      edge[1],
      edge[2].id,
      edge[3]
    ]) as Graph['edges'];
    const serialized = {
      id: this.graph.id,
      settings: $state.snapshot(this.settings),
      meta: $state.snapshot(this.graph.meta),
      nodes,
      edges
    };
    logger.log('serializing graph', serialized);
    return clone($state.snapshot(serialized));
  }

  private lastSettingsHash = 0;
  setSettings(settings: Record<string, unknown>) {
    const hash = fastHashString(JSON.stringify(settings));
    if (hash === this.lastSettingsHash) return;
    this.lastSettingsHash = hash;

    this.settings = settings;
    this.save();
    this.execute();
  }

  getNodeDefinitions() {
    return this.registry.getAllNodes();
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

  dropNodeOnEdge(nodeId: number, edge: Edge) {
    const draggedNode = this.getNode(nodeId);
    if (!draggedNode || !draggedNode.state?.type) return;

    const [fromNode, fromSocketIdx, toNode, toSocketKey] = edge;

    const draggedInputs = Object.entries(draggedNode.state.type.inputs ?? {});
    const draggedOutputs = draggedNode.state.type.outputs ?? [];

    const edgeOutputSocketType = fromNode.state?.type?.outputs?.[fromSocketIdx];
    const targetInput = toNode.state?.type?.inputs?.[toSocketKey];
    const targetAcceptedTypes = [targetInput?.type, ...(targetInput?.accepts || [])];

    const bestInputEntry = draggedInputs.find(([, input]) => {
      const accepted = [input.type, ...(input.accepts || [])];
      return areSocketsCompatible(edgeOutputSocketType, accepted);
    });

    const bestOutputIdx = draggedOutputs.findIndex(outputType =>
      areSocketsCompatible(outputType, targetAcceptedTypes)
    );

    if (!bestInputEntry || bestOutputIdx === -1) {
      logger.error('Could not find compatible sockets for drop');
      return;
    }

    this.startUndoGroup();

    this.removeEdge(edge, { applyDeletion: false });

    this.createEdge(fromNode, fromSocketIdx, draggedNode, bestInputEntry[0], {
      applyUpdate: false
    });

    this.createEdge(draggedNode, bestOutputIdx, toNode, toSocketKey, {
      applyUpdate: false
    });

    this.saveUndoGroup();
    this.execute();
  }

  getPossibleDropOnEdges(nodeId: number): Edge[] {
    const draggedNode = this.getNode(nodeId);
    if (!draggedNode || !draggedNode.state?.type) return [];

    const draggedInputs = Object.values(draggedNode.state.type.inputs ?? {});
    const draggedOutputs = draggedNode.state.type.outputs ?? [];

    // Optimization: Pre-calculate parents to avoid cycles
    const parentIds = new SvelteSet(this.getParentsOfNode(draggedNode).map(n => n.id));

    return this.edges.filter((edge) => {
      const [fromNode, fromSocketIdx, toNode, toSocketKey] = edge;

      // 1. Prevent cycles: If the target node is already a parent, we can't drop here
      if (parentIds.has(toNode.id)) return false;

      // 2. Prevent self-dropping: Don't drop on edges already connected to this node
      if (fromNode.id === nodeId || toNode.id === nodeId) return false;

      // 3. Check if edge.source can plug into ANY draggedNode.input
      const edgeOutputSocketType = fromNode.state?.type?.outputs?.[fromSocketIdx];
      const canPlugIntoDragged = draggedInputs.some(input => {
        const acceptedTypes = [input.type, ...(input.accepts || [])];
        return areSocketsCompatible(edgeOutputSocketType, acceptedTypes);
      });

      if (!canPlugIntoDragged) return false;

      // 4. Check if ANY draggedNode.output can plug into edge.target
      const targetInput = toNode.state?.type?.inputs?.[toSocketKey];
      const targetAcceptedTypes = [targetInput?.type, ...(targetInput?.accepts || [])];

      const draggedCanPlugIntoTarget = draggedOutputs.some(outputType =>
        areSocketsCompatible(outputType, targetAcceptedTypes)
      );

      return draggedCanPlugIntoTarget;
    });
  }

  getEdgesBetweenNodes(nodes: NodeInstance[]): [number, number, number, string][] {
    const edges = [];
    for (const node of nodes) {
      const children = node.state?.children || [];
      for (const child of children) {
        if (nodes.includes(child)) {
          const edge = this.edges.find(
            (e) => e[0].id === node.id && e[2].id === child.id
          );
          if (edge) {
            edges.push([edge[0].id, edge[1], edge[2].id, edge[3]] as [
              number,
              number,
              number,
              string
            ]);
          }
        }
      }
    }

    return edges;
  }

  private _init(graph: Graph) {
    const nodes = new SvelteMap(
      graph.nodes.map((node) => {
        return [node.id, node as NodeInstance];
      })
    );

    this.edges = graph.edges.map((edge) => {
      const from = nodes.get(edge[0]);
      const to = nodes.get(edge[2]);
      if (!from || !to) {
        throw new Error('Edge references non-existing node');
      }
      from.state.children = from.state.children || [];
      from.state.children.push(to);
      to.state.parents = to.state.parents || [];
      to.state.parents.push(from);
      return [from, edge[1], to, edge[3]] as Edge;
    });

    this.nodes.clear();
    for (const [id, node] of nodes) {
      this.nodes.set(id, node);
    }

    this.execute();
  }

  private async loadAllCollections() {
    // Fetch all nodes from all collections of the loaded nodes
    const nodeIds = Array.from(new Set([...this.graph.nodes.map((n) => n.type)]));
    const allCollections = new Set<`${string}/${string}`>();
    for (const id of nodeIds) {
      const [user, collection] = id.split('/');
      allCollections.add(`${user}/${collection}`);
    }

    const allCollectionIds = await Promise
      .all([...allCollections]
        .map(async (collection) =>
          remoteRegistry
            .fetchCollection(collection)
            .then((collection: { nodes: { id: NodeId }[] }) => {
              return collection.nodes.map(n => n.id.replace(/\.wasm$/, '') as NodeId);
            })
        ));

    const missingNodeIds = [...new Set(allCollectionIds.flat())];

    this.registry.load(missingNodeIds);
  }

  async load(graph: Graph) {
    const a = performance.now();

    this.loaded = false;
    this.graph = graph;
    this.status = 'loading';
    this.id = graph.id;

    logger.info('loading graph', { nodes: graph.nodes, edges: graph.edges, id: graph.id });

    const nodeIds = Array.from(new SvelteSet([...graph.nodes.map((n) => n.type)]));
    await this.registry.load(nodeIds);

    // Fetch all nodes from all collections of the loaded nodes
    const allCollections = new SvelteSet<`${string}/${string}`>();
    for (const id of nodeIds) {
      const [user, collection] = id.split('/');
      allCollections.add(`${user}/${collection}`);
    }
    for (const collection of allCollections) {
      remoteRegistry
        .fetchCollection(collection)
        .then((collection: { nodes: { id: NodeId }[] }) => {
          const ids = collection.nodes.map((n) => n.id);
          return this.registry.load(ids);
        });
    }

    logger.info('loaded node types', this.registry.getAllNodes());

    for (const node of this.graph.nodes) {
      const nodeType = this.registry.getNode(node.type);
      if (!nodeType) {
        logger.error(`Node type not found: ${node.type}`);
        this.status = 'error';
        return;
      }
      // Turn into runtime node
      const n = node as NodeInstance;
      n.state = {};
      n.state.type = nodeType;
    }

    // load settings
    const settingTypes: Record<
      string,
      // Optional metadata to map settings to specific nodes
      NodeInput & { __node_type: string; __node_input: string }
    > = {};
    const settingValues = graph.settings || {};
    const types = this.getNodeDefinitions();
    for (const type of types) {
      if (type.inputs) {
        for (const key in type.inputs) {
          const settingId = type.inputs[key].setting;
          if (settingId) {
            settingTypes[settingId] = {
              __node_type: type.id,
              __node_input: key,
              ...type.inputs[key]
            };
            if (
              settingValues[settingId] === undefined
              && 'value' in type.inputs[key]
            ) {
              settingValues[settingId] = type.inputs[key].value;
            }
          }
        }
      }
    }

    this.settings = settingValues;
    this.emit('settings', { types: settingTypes, values: settingValues });

    this.history.reset();
    this._init(this.graph);

    this.save();

    this.status = 'idle';

    this.loaded = true;
    logger.log(`Graph loaded in ${performance.now() - a}ms`);

    setTimeout(() => this.execute(), 100);
    this.loadAllCollections(); // lazily load all nodes from all collections
  }

  getAllNodes() {
    return Array.from(this.nodes.values());
  }

  getNode(id: number) {
    return this.nodes.get(id);
  }

  getNodeType(id: string) {
    return this.registry.getNode(id);
  }

  async loadNodeType(id: NodeId) {
    await this.registry.load([id]);
    const nodeType = this.registry.getNode(id);

    if (!nodeType) return;

    const settingTypes = this.settingTypes;
    const settingValues = this.settings;
    if (nodeType.inputs) {
      for (const key in nodeType.inputs) {
        const settingId = nodeType.inputs[key].setting;
        if (settingId) {
          settingTypes[settingId] = nodeType.inputs[key];
          if (
            settingValues
            && settingValues?.[settingId] === undefined
            && 'value' in nodeType.inputs[key]
          ) {
            settingValues[settingId] = nodeType.inputs[key].value;
          }
        }
      }
    }

    this.settings = settingValues;
    this.settingTypes = settingTypes;
    this.emit('settings', { types: settingTypes, values: settingValues });
  }

  getChildren(node: NodeInstance) {
    const children = [];
    const stack = node.state?.children?.slice(0);
    while (stack?.length) {
      const child = stack.pop();
      if (!child) continue;
      children.push(child);
      stack.push(...(child.state?.children || []));
    }
    return children;
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
    const edgesToNode = this.edges.filter((edge) => edge[2].id === node.id);
    const edgesFromNode = this.edges.filter((edge) => edge[0].id === node.id);
    for (const edge of [...edgesToNode, ...edgesFromNode]) {
      this.removeEdge(edge, { applyDeletion: false });
    }

    if (restoreEdges) {
      const outputSockets = edgesToNode.map((e) => [e[0], e[1]] as const);
      const inputSockets = edgesFromNode.map((e) => [e[2], e[3]] as const);

      for (const [to, toSocket] of inputSockets) {
        for (const [from, fromSocket] of outputSockets) {
          const outputType = from.state?.type?.outputs?.[fromSocket];
          const inputType = to?.state?.type?.inputs?.[toSocket]?.type;
          if (outputType === inputType) {
            this.createEdge(from, fromSocket, to, toSocket, {
              applyUpdate: false
            });
            continue;
          }
        }
      }
    }

    this.nodes.delete(node.id);
    this.execute();
    this.save();
  }

  smartConnect(from: NodeInstance, to: NodeInstance): Edge | undefined {
    const inputs = Object.entries(to.state?.type?.inputs ?? {});
    const outputs = from.state?.type?.outputs ?? [];
    for (let i = 0; i < inputs.length; i++) {
      const [inputName, input] = inputs[i];
      for (let o = 0; o < outputs.length; o++) {
        const output = outputs[o];
        if (input.type === output || input.type === '*') {
          return this.createEdge(from, o, to, inputName);
        }
      }
    }
  }

  createNodeId() {
    return Math.max(0, ...this.nodes.keys()) + 1;
  }

  createGraph(nodes: NodeInstance[], edges: [number, number, number, string][]) {
    // map old ids to new ids
    const idMap = new SvelteMap<number, number>();

    let startId = this.createNodeId();

    nodes = nodes.map((node) => {
      const id = startId++;
      idMap.set(node.id, id);
      const type = this.registry.getNode(node.type);
      if (!type) {
        throw new Error(`Node type not found: ${node.type}`);
      }
      return { ...node, id, tmp: { type } };
    });

    const _edges = edges.map((edge) => {
      const from = nodes.find((n) => n.id === idMap.get(edge[0]));
      const to = nodes.find((n) => n.id === idMap.get(edge[2]));

      if (!from || !to) {
        throw new Error('Edge references non-existing node');
      }

      to.state.parents = to.state.parents || [];
      to.state.parents.push(from);

      from.state.children = from.state.children || [];
      from.state.children.push(to);

      return [from, edge[1], to, edge[3]] as Edge;
    });

    for (const node of nodes) {
      this.nodes.set(node.id, node);
    }

    this.edges.push(..._edges);

    this.save();
    return nodes;
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
    if (!nodeType) {
      logger.error(`Node type not found: ${type}`);
      return;
    }

    const node: NodeInstance = $state({
      id: this.createNodeId(),
      type,
      position,
      state: { type: nodeType },
      props
    });

    this.nodes.set(node.id, node);

    this.save();

    return node;
  }

  createEdge(
    from: NodeInstance,
    fromSocket: number,
    to: NodeInstance,
    toSocket: string,
    { applyUpdate = true } = {}
  ): Edge | undefined {
    const existingEdges = this.getEdgesToNode(to);

    // check if this exact edge already exists
    const existingEdge = existingEdges.find(
      (e) => e[0].id === from.id && e[1] === fromSocket && e[3] === toSocket
    );
    if (existingEdge) {
      logger.error('Edge already exists', existingEdge);
      return;
    }

    const fromType = from.state.type || this.registry.getNode(from.type);
    const toType = to.state.type || this.registry.getNode(to.type);

    // check if socket types match
    const fromSocketType = fromType?.outputs?.[fromSocket];
    const toSocketType = [toType?.inputs?.[toSocket]?.type];
    if (toType?.inputs?.[toSocket]?.accepts) {
      toSocketType.push(...(toType?.inputs?.[toSocket]?.accepts || []));
    }

    if (!areSocketsCompatible(fromSocketType, toSocketType)) {
      logger.error(
        `Socket types do not match: ${fromSocketType} !== ${toSocketType}`
      );
      return;
    }

    const edgeToBeReplaced = this.edges.find(
      (e) => e[2].id === to.id && e[3] === toSocket
    );
    if (edgeToBeReplaced) {
      this.removeEdge(edgeToBeReplaced, { applyDeletion: false });
    }

    const edge = [from, fromSocket, to, toSocket] as Edge;

    this.edges.push(edge);

    from.state.children = from.state.children || [];
    from.state.children.push(to);
    to.state.parents = to.state.parents || [];
    to.state.parents.push(from);

    if (applyUpdate) {
      this.save();
    }
    this.execute();

    return edge;
  }

  undo() {
    const nextState = this.history.undo();
    if (nextState) {
      this._init(nextState);
      this.emit('save', this.serialize());
    }
  }

  redo() {
    const nextState = this.history.redo();
    if (nextState) {
      this._init(nextState);
      this.emit('save', this.serialize());
    }
  }

  startUndoGroup() {
    this.currentUndoGroup = 1;
  }

  saveUndoGroup() {
    this.currentUndoGroup = null;
    this.save();
  }

  save() {
    if (this.currentUndoGroup) return;
    const state = this.serialize();
    this.history.save(state);

    // This is some stupid race condition where the graph-manager emits a save event
    // when the graph is not fully loaded
    if (this.nodes.size === 0 && this.edges.length === 0) {
      return;
    }

    this.emit('save', state);
    logger.log('saving graphs', state);
  }

  getParentsOfNode(node: NodeInstance) {
    const parents = [];
    const stack = node.state?.parents?.slice(0);
    while (stack?.length) {
      if (parents.length > 1000000) {
        logger.warn('Infinite loop detected');
        break;
      }
      const parent = stack.pop();
      if (!parent) continue;
      parents.push(parent);
      stack.push(...(parent.state?.parents || []));
    }
    return parents.reverse();
  }

  getPossibleNodes(socket: Socket): NodeDefinition[] {
    const allDefinitions = this.getNodeDefinitions();

    const nodeType = socket.node.state?.type;
    if (!nodeType) {
      return [];
    }

    const definitions = typeof socket.index === 'string'
      ? allDefinitions.filter(s => {
        return s.outputs?.find(_s =>
          Object
            .values(nodeType?.inputs || {})
            .map(s => s.type)
            .includes(_s as NodeInput['type'])
        );
      })
      : allDefinitions.filter(s =>
        Object
          .values(s.inputs ?? {})
          .find(s => {
            if (s.hidden) return false;
            if (nodeType.outputs?.includes(s.type)) {
              return true;
            }
            return s.accepts?.find(a => nodeType.outputs?.includes(a));
          })
      );

    return definitions;
  }

  getPossibleSockets({ node, index }: Socket): [NodeInstance, string | number][] {
    const nodeType = this.registry.getNode(node.type);
    if (!nodeType) return [];
    console.log({ index });

    const sockets: [NodeInstance, string | number][] = [];

    // if index is a string, we are an input looking for outputs
    if (typeof index === 'string') {
      // filter out self and child nodes
      const children = new SvelteSet(this.getChildren(node).map((n) => n.id));
      const nodes = this.getAllNodes().filter(
        (n) => n.id !== node.id && !children.has(n.id)
      );

      const ownType = nodeType?.inputs?.[index].type;

      for (const node of nodes) {
        const nodeType = this.registry.getNode(node.type);
        const inputs = nodeType?.outputs;
        if (!inputs) continue;
        for (let index = 0; index < inputs.length; index++) {
          if (inputs[index] === ownType) {
            sockets.push([node, index]);
          }
        }
      }
    } else if (typeof index === 'number') {
      // if index is a number, we are an output looking for inputs

      // filter out self and parent nodes
      const parents = new SvelteSet(this.getParentsOfNode(node).map((n) => n.id));
      const nodes = this.getAllNodes().filter(
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

      const ownType = nodeType.outputs?.[index];

      for (const node of nodes) {
        const inputs = this.registry.getNode(node.type)?.inputs;
        if (!inputs) continue;
        for (const key in inputs) {
          const otherType = [inputs[key].type];
          otherType.push(...(inputs[key].accepts || []));

          if (
            areSocketsCompatible(ownType, otherType)
            && !edges.get(node.id)?.includes(key)
          ) {
            sockets.push([node, key]);
          }
        }
      }
    }

    console.log(`Found ${sockets.length} possible sockets`, sockets);
    return sockets;
  }

  removeEdge(
    edge: Edge,
    { applyDeletion = true }: { applyDeletion?: boolean } = {}
  ) {
    const id0 = edge[0].id;
    const sid0 = edge[1];
    const id2 = edge[2].id;
    const sid2 = edge[3];

    const _edge = this.edges.find(
      (e) => e[0].id === id0 && e[1] === sid0 && e[2].id === id2 && e[3] === sid2
    );

    if (!_edge) return;

    if (edge[0].state.children) {
      edge[0].state.children = edge[0].state.children.filter(
        (n: NodeInstance) => n.id !== id2
      );
    }

    if (edge[2].state.parents) {
      edge[2].state.parents = edge[2].state.parents.filter(
        (n: NodeInstance) => n.id !== id0
      );
    }

    this.edges = this.edges.filter((e) => !areEdgesEqual(e, edge));
    if (applyDeletion) {
      this.execute();
      this.save();
    }
  }

  getEdgesToNode(node: NodeInstance) {
    return this.edges
      .filter((edge) => edge[2].id === node.id)
      .map((edge) => {
        const from = this.getNode(edge[0].id);
        const to = this.getNode(edge[2].id);
        if (!from || !to) return;
        return [from, edge[1], to, edge[3]] as const;
      })
      .filter(Boolean) as unknown as [NodeInstance, number, NodeInstance, string][];
  }

  getEdgesFromNode(node: NodeInstance) {
    return this.edges
      .filter((edge) => edge[0].id === node.id)
      .map((edge) => {
        const from = this.getNode(edge[0].id);
        const to = this.getNode(edge[2].id);
        if (!from || !to) return;
        return [from, edge[1], to, edge[3]] as const;
      })
      .filter(Boolean) as unknown as [NodeInstance, number, NodeInstance, string][];
  }
}
