import throttle from '$lib/helpers/throttle';
import { RemoteNodeRegistry } from '$lib/node-registry/index';
import type {
  Box,
  Edge,
  Graph,
  GroupDefinition,
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

const clone = 'structuredClone' in globalThis
  ? globalThis.structuredClone
  : (args: unknown) => JSON.parse(JSON.stringify(args));

function areSocketsCompatible(
  output: string | undefined,
  inputs: string | (string | undefined)[] | undefined
) {
  if (output === '*') return true;
  if (Array.isArray(inputs) && output) {
    return inputs.includes('*') || inputs.includes(output);
  }
  return inputs === output;
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

  graph: Graph = $state({ id: 0, nodes: [], edges: [], groups: [] });
  id = $state(0);

  nodes = new SvelteMap<number, NodeInstance>();
  nodeArray = $derived(Array.from(this.nodes.values()));

  edges = $state<Edge[]>([]);

  // Plain array — NOT $state. rootGraph items are plain-serialized (safe for structuredClone).
  // savedNodes/savedEdges hold live reactive references so reactivity is preserved on exit.
  graphStack: {
    rootGraph: Graph;
    savedNodes: Map<number, NodeInstance>;
    savedEdges: Edge[];
    outerGraph: Graph;
    groupId: number;
    nodeId: number;
    cameraPosition: [number, number, number];
  }[] = [];

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

  public serializeFullGraph(): Graph {
    if (this.graphStack.length === 0) return this.serialize();
    let merged = this.serialize();
    for (let i = this.graphStack.length - 1; i >= 0; i--) {
      const { rootGraph, groupId } = this.graphStack[i];
      merged = {
        ...rootGraph,
        groups: rootGraph.groups.map(g =>
          g.id === groupId ? { ...g, nodes: merged.nodes, edges: merged.edges } : g
        )
      };
    }
    return merged;
  }

  execute = throttle(() => {
    if (this.loaded === false) return;
    this.emit('result', this.serializeFullGraph());
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

    const groups = this.graph.groups?.map((group) => {
      const groupNodes = group.nodes.map((node) => ({
        id: node.id,
        position: [...node.position],
        type: node.type,
        props: node.props
      })) as NodeInstance[];

      return {
        id: group.id,
        name: group.name,
        inputs: group.inputs,
        outputs: group.outputs,
        nodes: groupNodes,
        edges: group.edges
      };
    });

    const serialized = {
      id: this.graph.id,
      settings: $state.snapshot(this.settings),
      meta: $state.snapshot(this.graph.meta),
      groups,
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

  tryConnectToDebugNode(nodeId: number) {
    const node = this.nodes.get(nodeId);
    if (!node) return;
    if (node.type.endsWith('/debug')) return;
    if (!node.state.type?.outputs?.length) return;
    let debugNode = this.nodes.values().find(n => n.type.endsWith('/debug'));

    if (!debugNode) {
      debugNode = this.createNode({
        type: '__internal/node/debug',
        position: [node.position[0] + 30, node.position[1]],
        props: {}
      });
    }

    if (debugNode) {
      this.createEdge(node, 0, debugNode, 'input');
    }

    return debugNode;
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
        const n = node as NodeInstance;
        const registryType = this.registry.getNode(node.type);
        n.state = registryType ? { type: registryType } : {};
        const resolvedType = this.getNodeType(n);
        if (resolvedType) n.state = { type: resolvedType };
        return [node.id, n];
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

  async load(graph: Graph) {
    const a = performance.now();

    this.loaded = false;
    graph.groups ??= [];
    this.graph = graph;
    this.status = 'loading';
    this.id = graph.id;

    logger.info(
      'loading graph',
      { nodes: graph.nodes, edges: graph.edges, id: graph.id }
    );

    const nodeIds = Array
      .from(
        new SvelteSet(
          [
            ...graph.nodes,
            graph?.groups?.map(g => g.nodes).flat()
          ]
            .filter(n => n && 'type' in n)
            .map((n) => n.type)
        )
      )
      .filter(n => !n.startsWith('__internal/'));

    await this.registry.load(nodeIds);

    // Fetch all nodes from all collections of the loaded nodes
    const allCollections = new SvelteSet<`${string}/${string}`>();
    for (const id of nodeIds) {
      const [user, collection] = id.split('/');
      if (user === '__internal') continue;
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
      if (!nodeType && !node.type.startsWith('__internal/')) {
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
  }

  getAllNodes() {
    return Array
      .from(this.nodes.values());
  }

  getNode(id: number) {
    return this.nodes.get(id);
  }

  getNodeType(node: NodeInstance) {
    if (!node) {
      console.trace('failed to get node type', { node });
      return;
    }

    if (node.type === '__internal/group/input') {
      const groupId = this.graphStack.at(-1)?.groupId;
      const group = groupId !== undefined ? this.getGroup(groupId) : undefined;
      if (!group) return node.state.type;

      const groupInputs: NodeDefinition['inputs'] = Object.fromEntries(
        Object.values(group?.inputs || {}).map((o, i) => {
          return [
            `in_${i}`,
            {
              ...o,
              external: true
            }
          ];
        }) || []
      );
      return {
        id: '__internal/group/input' as NodeId,
        meta: {
          title: 'Group Input'
        },
        inputs: groupInputs,
        execute: (x: Int32Array) => x
      } as NodeDefinition;
    }

    if (node.type === '__internal/group/output') {
      const groupId = this.graphStack.at(-1)?.groupId;
      const group = groupId !== undefined ? this.getGroup(groupId) : undefined;
      if (!group) return node.state.type;
      return {
        id: '__internal/group/output' as NodeId,
        inputs: Object.fromEntries(
          (group.outputs ?? []).map((
            o,
            i
          ) => [`out_${i}`, { type: o.type, label: o.label, external: true }])
        ),
        meta: {
          title: 'Group Output'
        },
        outputs: [],
        execute: (x: Int32Array) => x
      } as NodeDefinition;
    }

    // Construct the group inputs on the fly
    if (node.type === '__internal/group/instance') {
      const groupDefinition = this.getGroup(node.props?.groupId as number);

      if (!groupDefinition) {
        logger.error(`Group not found: ${node.props?.groupId}`);
        return;
      }

      const defaultInputs = {
        ...(node.state.type?.inputs || {}),
        ...groupDefinition?.inputs
      };

      // This is to make sure the the groupId is always first
      delete defaultInputs['groupId'];
      const inputs = {
        'groupId': {
          type: 'select',
          label: 'Group',
          value: node.props?.groupId,
          internal: true,
          options: this.graph.groups.map((g, i) => ({
            value: g.id,
            label: g.name || `Group ${i + 1}`
          }))
        },
        ...defaultInputs
      };

      const groupType = {
        ...node.state.type,
        meta: {
          title: 'Group',
          ...node?.state?.type?.meta || {}
        },
        inputs,
        outputs: groupDefinition?.outputs?.map(o => o.type)
      } as NodeDefinition;

      return groupType;
    }

    return node.state.type;
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
      const [inputName, input] = inputs[0];
      for (let o = 0; o < outputs.length; o++) {
        const output = outputs[0];
        if (input.type === output) {
          return this.createEdge(from, o, to, inputName);
        }
      }
    }
  }

  getGroup(id: number) {
    return this.graph.groups.find(g => g.id === id);
  }

  renameGroup(groupId: number, name: string) {
    const group = this.getGroup(groupId);
    if (!group) return;
    group.name = name;
    this.save();
  }

  isInsideGroup = $state(false);

  enterGroup(nodeId: number, cameraPosition: [number, number, number]): boolean {
    const groupNode = this.getNode(nodeId);
    if (!groupNode || groupNode.type !== '__internal/group/instance') return false;
    const groupId = groupNode.props?.groupId as number;
    const group = this.getGroup(groupId);
    if (!group) return false;

    this.graphStack.push({
      rootGraph: this.serialize(),
      savedNodes: new SvelteMap(this.nodes),
      savedEdges: [...this.edges],
      outerGraph: this.graph,
      groupId,
      nodeId,
      cameraPosition
    });
    this.graph = { ...this.graph, nodes: group.nodes, edges: group.edges };
    this._init(this.graph);
    this.history.reset();
    this.isInsideGroup = true;
    return true;
  }

  exitGroup(): { camera: [number, number, number]; nodeId: number } | false {
    if (!this.graphStack.length) return false;
    const { savedNodes, savedEdges, outerGraph, groupId, nodeId, cameraPosition } = this.graphStack
      .pop()!;
    const internalState = this.serialize();

    // Clear stale DOM/mesh refs so the remounting components register fresh ones.
    // The $effect guards in NodeHTML/Node only set these when undefined, so without
    // this clear they'd keep pointing to the detached elements from before group entry.
    for (const node of savedNodes.values()) {
      node.state.ref = undefined;
      node.state.mesh = undefined;
    }

    // Restore live reactive nodes and edges so drag-reactivity is preserved
    this.nodes.clear();
    for (const [id, node] of savedNodes) {
      this.nodes.set(id, node);
    }
    this.edges = savedEdges;

    // Patch the group definition with the edited internal graph
    this.graph = {
      ...outerGraph,
      groups: (outerGraph.groups ?? []).map(g =>
        g.id === groupId ? { ...g, nodes: internalState.nodes, edges: internalState.edges } : g
      )
    };

    this.history.reset();
    this.isInsideGroup = this.graphStack.length > 0;
    this.execute();
    this.save();
    return { camera: cameraPosition, nodeId };
  }

  createNodeId() {
    const ids = [
      ...this.nodes.keys(),
      ...this.graph.groups.map(g => g.id),
      ...this.graph.groups.flatMap(g => g.nodes.map(n => n.id))
    ];

    let id = 0;
    while (ids.includes(id)) {
      id++;
    }
    return id;
  }

  createGraph(nodes: NodeInstance[], edges: [number, number, number, string][]) {
    // map old ids to new ids
    const idMap = new SvelteMap<number, number>();

    let startId = this.createNodeId();

    nodes = nodes.map((node) => {
      const id = startId++;
      idMap.set(node.id, id);
      const type = this.registry.getNode(node.type);
      if (!type && !node.type.startsWith('__internal/')) {
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

  removeUnusedGroups() {
    const usedGroups = new SvelteSet(this.getAllNodes().map(n => n.props?.groupId));
    const unusedGroupAmount = this.graph.groups.length - usedGroups.size;
    this.graph.groups = this.graph.groups.filter(g => usedGroups.has(g.id));
    this.save();
    return unusedGroupAmount;
  }

  groupNodes(nodeIds: number[]) {
    this.startUndoGroup();
    this.removeUnusedGroups();

    const nodes = [
      ...new SvelteSet(nodeIds).values().map(id => this.getNode(id)).filter(Boolean)
    ] as NodeInstance[];

    if (!nodes.length) return;

    logger.log(`Grouping ${nodes.length} nodes`, { nodes });

    const ids = new SvelteSet(nodes.map(n => n.id));

    // We use the map to dedupe when one external node is connected to multiple internal nodes
    //           ┌──internal_a
    // external──┤
    //           └──internal_b
    // This should only result in one group input not two
    const incomingEdges = this.edges.filter((edge) => ids.has(edge[2].id) && !ids.has(edge[0].id));
    const groupInputs = new SvelteMap<string, Edge>();
    for (const edge of incomingEdges) {
      groupInputs.set(`${edge[0].id}-${edge[1]}`, edge);
    }

    // And the same for the outputs
    const outgoingEdges = this.edges.filter((edge) => ids.has(edge[0].id) && !ids.has(edge[2].id));
    const groupOutputs = new SvelteMap<string, Edge>();
    for (const edge of outgoingEdges) {
      groupOutputs.set(`${edge[2].id}-${edge[3]}`, edge);
    }

    const inputs: Record<string, NodeInput> = {};
    [...groupInputs.values()].forEach((edge, i) => {
      const input = {
        label: `Input ${i}`,
        type: edge[0].state.type?.outputs?.[edge[1]] || '*'
      };
      inputs[`input_${i}`] = input as NodeInput;
    });

    const outputs = [...groupOutputs.values()].map((edge, i) => ({
      label: `Output ${i}`,
      type: edge[2].state.type?.inputs?.[edge[3]].type || '*'
    }));

    const groupPosition = [0, 0] as [number, number];
    const bounds: Box = { minX: Infinity, maxX: -Infinity, minY: Infinity, maxY: -Infinity };
    for (const node of nodes) {
      groupPosition[0] += node.position[0];
      groupPosition[1] += node.position[1];
      bounds.minX = Math.min(bounds.minX, node.position[0]);
      bounds.maxX = Math.max(bounds.maxX, node.position[0]);
      bounds.minY = Math.min(bounds.minY, node.position[1]);
      bounds.maxY = Math.max(bounds.maxY, node.position[1]);
    }
    groupPosition[0] /= nodes.length;
    groupPosition[1] /= nodes.length;

    // Map from deduped edge source key → group input index, used for both
    // internal edge wiring and external edge socket naming.
    const inputIndexByEdgeKey = new SvelteMap<string, number>();
    [...groupInputs.keys()].forEach((key, i) => inputIndexByEdgeKey.set(key, i));

    // Allocate all needed IDs up front so sequential calls never collide.
    const usedIds = new SvelteSet<number>([
      ...this.nodes.keys(),
      ...this.graph.groups.map(g => g.id),
      ...this.graph.groups.flatMap(g => g.nodes.map(n => n.id))
    ]);
    const nextId = () => {
      let id = 0;
      while (usedIds.has(id)) id++;
      usedIds.add(id);
      return id;
    };

    const groupInputNode: NodeInstance = {
      id: nextId(),
      type: '__internal/group/input',
      position: [bounds.minX - 50, (bounds.minY + bounds.maxY) / 2],
      state: {}
    };

    const groupOutputNode: NodeInstance = {
      id: nextId(),
      type: '__internal/group/output',
      position: [bounds.maxX + 25, (bounds.minY + bounds.maxY) / 2],
      state: {}
    };

    // Edges that are inside the group, routed through boundary nodes at
    // the correct input/output index for each unique external connection.
    const internalEdges = this.edges.filter((edge) => {
      return ids.has(edge[0].id) || ids.has(edge[2].id);
    }).map((edge) => {
      if (!ids.has(edge[0].id)) {
        const idx = inputIndexByEdgeKey.get(`${edge[0].id}-${edge[1]}`) ?? 0;
        return [groupInputNode.id, idx, edge[2].id, edge[3]];
      } else if (!ids.has(edge[2].id)) {
        return [edge[0].id, edge[1], groupOutputNode.id, 'out_0'];
      }
      return [edge[0].id, edge[1], edge[2].id, edge[3]];
    }) as [number, number, number, string][];

    const groupId = nextId();
    const groupDefinition: GroupDefinition = {
      id: groupId,
      inputs: inputs,
      outputs: outputs,
      edges: internalEdges,
      nodes: [groupInputNode, ...nodes, groupOutputNode]
    };

    // Push before createNode so createNodeId() inside sees the allocated IDs.
    this.graph.groups.push(groupDefinition);

    const groupNode = this.createNode({
      type: '__internal/group/instance',
      position: [groupPosition[0], groupPosition[1]],
      props: {
        groupId: groupId
      }
    });

    if (!groupNode) throw new Error('Failed to create group node');

    // Rewire external edges to/from the group node using the correct input socket.
    const externalEdges = this.edges.map((edge) => {
      if (ids.has(edge[2].id)) {
        const idx = inputIndexByEdgeKey.get(`${edge[0].id}-${edge[1]}`) ?? 0;
        return [edge[0], edge[1], groupNode, `input_${idx}`] as Edge;
      } else if (ids.has(edge[0].id)) {
        return [groupNode, 0, edge[2], edge[3]] as Edge;
      }
      return edge;
    });

    this.nodes.set(groupNode.id, groupNode);
    this.edges = externalEdges;

    // Remove nodes from graph which are not part of the group
    for (const node of nodes) {
      this.removeNode(node);
    }

    this.saveUndoGroup();

    return groupNode;
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
    if (!nodeType && !type.startsWith('__internal/')) {
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

    const fromType = this.getNodeType(from);
    const toType = this.getNodeType(to);

    // check if socket types match
    const fromSocketType = from.type === '__internal/group/input'
      ? fromType?.inputs?.[Object.keys(fromType?.inputs || {})[fromSocket]].type
      : fromType?.outputs?.[fromSocket];
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

    const fullState = this.graphStack.length > 0 ? this.serializeFullGraph() : state;
    this.emit('save', fullState);
    logger.log('saving graphs', fullState);
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
    const nodeType = this.getNodeType(node);
    if (!nodeType) return [];

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
        const nodeType = this.getNodeType(node);
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

      const ownType = node.type === '__internal/group/input'
        ? nodeType.inputs?.[Object.keys(nodeType?.inputs || {})[index]].type
        : nodeType.outputs?.[index];

      for (const node of nodes) {
        const inputs = this.getNodeType(node)?.inputs;
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
