import { clone } from '$lib/helpers';
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
  SerializedEdge,
  SerializedNode,
  Socket
} from '@nodarium/types';
import { fastHashString } from '@nodarium/utils';
import { createLogger } from '@nodarium/utils';
import { SvelteMap, SvelteSet } from 'svelte/reactivity';
import EventEmitter from './helpers/EventEmitter';
import {
  areEdgesEqual,
  areSocketsCompatible,
  serializeEdge,
  serializeNode
} from './helpers/nodeHelpers';
import { HistoryManager } from './history-manager';

const log = createLogger('graph-manager');
log.mute();

const remoteRegistry = new RemoteNodeRegistry('');

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

  // Snapshots of parent levels we navigated away from. Empty at root.
  // Entry i has the saved state of depth i (0 = root graph, 1 = first group, …).
  parentStack: {
    id: number;
    nodes: SerializedNode[];
    edges: SerializedEdge[];
    nodeId: number; // group instance node id that was entered to reach the next level
  }[] = $state([]);

  // ID of the currently active group, or null when at the root graph.
  currentGroupId = $state<number | null>(null);

  // Graph Data
  id = $state(0);
  meta = $state<Graph['meta']>({});
  nodes = new SvelteMap<number, NodeInstance>();
  edges = $state<Edge[]>([]);
  groups: GroupDefinition[] = $state([]);

  nodeArray = $derived(Array.from(this.nodes.values()));

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
    const nodes =
      (this.parentStack.length === 0 ? Array.from(this.nodes.values()) : this.parentStack[0].nodes)
        .map(n => serializeNode(n));
    const edges =
      (this.parentStack.length === 0 ? Array.from(this.edges.values()) : this.parentStack[0].edges)
        .map(e => serializeEdge(e));

    const groups = this.groups?.map((group) => {
      const isCurrentActive = this.currentGroupId === group.id;
      const stackState = this.parentStack.find((s) => s.id === group.id);
      const groupNodes =
        (isCurrentActive ? [...this.nodes.values()] : stackState?.nodes ?? group.nodes).map(
          n => serializeNode(n)
        );
      const groupEdges =
        (isCurrentActive ? [...this.edges.values()] : stackState?.edges ?? group.edges).map(
          e => serializeEdge(e)
        );
      return {
        id: group.id,
        name: group.name,
        inputs: group.inputs,
        outputs: group.outputs,
        nodes: groupNodes,
        edges: groupEdges
      };
    });

    const serialized = $state.snapshot({
      id: this.id,
      settings: this.settings,
      meta: this.meta,
      groups,
      nodes,
      edges
    });
    log.log('serializing graph', serialized);
    return clone(serialized);
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
      log.error('Could not find compatible sockets for drop');
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

  private _init(
    graph: { nodes: SerializedNode[]; edges: SerializedEdge[] }
  ) {
    this.nodes.clear();
    for (const node of graph.nodes) {
      const n = $state(node) as NodeInstance;
      const registryType = this.registry.getNode(node.type);
      n.state = registryType ? { type: registryType } : {};
      const resolvedType = this.getNodeType(n);
      if (resolvedType) n.state = { type: resolvedType };
      this.nodes.set(n.id, n);
    }

    this.edges = graph.edges.map((edge) => {
      const from = this.nodes.get(edge[0]);
      const to = this.nodes.get(edge[2]);
      if (!from || !to) {
        throw new Error('Edge references non-existing node');
      }
      from.state.children = from.state.children || [];
      from.state.children.push(to);
      to.state.parents = to.state.parents || [];
      to.state.parents.push(from);
      return [from, edge[1], to, edge[3]] as Edge;
    });

    this.execute();
  }

  async load(graph: Graph) {
    const a = performance.now();

    this.loaded = false;
    graph.groups ??= [];
    this.meta = graph.meta;
    this.groups = graph.groups;
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
            graph?.groups?.map(g => g.nodes).flat()
          ]
            .filter(n => n && 'type' in n)
            .map((n) => n.type)
        )
      );

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

    log.info('loaded node types', this.registry.getAllNodes());

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

    this.parentStack = [];
    this.currentGroupId = null;

    this.settings = settingValues;
    this.emit('settings', { types: settingTypes, values: settingValues });

    this.history.reset();
    this._init(graph);
    this.save();
    this.status = 'idle';
    this.loaded = true;
    log.log(`Graph loaded in ${performance.now() - a}ms`);
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
      const group = this.currentGroupId !== null ? this.getGroup(this.currentGroupId) : undefined;
      if (!group) return node.state.type;

      const groupInputs: NodeDefinition['inputs'] = Object.fromEntries(
        Object.values(group?.inputs || {}).map((o, i) => {
          return [`in_${i}`, {
            ...o,
            external: true
          }];
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
      const group = this.currentGroupId !== null ? this.getGroup(this.currentGroupId) : undefined;
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
        log.error(`Group not found: ${node.props?.groupId}`);
        return;
      }

      const defaultInputs = {
        ...(node.state.type?.inputs || {}),
        ...groupDefinition?.inputs
      };

      delete defaultInputs['groupId'];

      const inputs = {
        'groupId': {
          type: 'select',
          label: '',
          value: node.props?.groupId,
          internal: true,
          options: this.groups.map((g) => ({
            value: g.id,
            label: g.name || `Group#${g.id}`
          })).filter((g) => {
            const activeIds = new SvelteSet([
              ...this.parentStack.filter(e => e.id !== this.id).map(e => e.id),
              ...(this.currentGroupId !== null ? [this.currentGroupId] : [])
            ]);
            return !activeIds.has(g.value);
          })
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
    log.log('removing node', { id: node.id, type: node.type, restoreEdges });
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
    return this.groups.find(g => g.id === id);
  }

  renameGroup(groupId: number, name: string) {
    log.log('renaming group', { groupId, name });
    const group = this.getGroup(groupId);
    if (!group) return;
    group.name = name;
    this.save();
  }

  isInsideGroup = $derived(this.currentGroupId !== null);

  enterGroup(nodeId: number): boolean {
    const groupNode = this.getNode(nodeId);
    if (!groupNode || groupNode.type !== '__internal/group/instance') return false;

    log.log('entering group', { nodeId });

    const groupId = groupNode.props?.groupId as number;
    const group = this.getGroup(groupId);
    if (!group) return false;

    // Snapshot current level and push it onto the parent stack.
    this.parentStack.push({
      id: this.currentGroupId ?? this.id,
      nodes: [...this.nodes.values()].map(n => serializeNode(n)),
      edges: [...this.edges.values()].map(e => serializeEdge(e)),
      nodeId
    });
    this.currentGroupId = groupId;

    log.log('entered group', { groupId, depth: this.parentStack.length });
    this.history.reset();
    this._init(group);
    return true;
  }

  exitGroup() {
    log.log('exiting group', { depth: this.parentStack.length });
    if (this.parentStack.length === 0) return;

    // Persist live edits back to the GroupDefinition.
    const group = this.getGroup(this.currentGroupId!);
    if (group) {
      group.nodes = [...this.nodes.values()].map(n => serializeNode(n));
      group.edges = [...this.edges.values()].map(e => serializeEdge(e));
    }

    const parent = this.parentStack.pop()!;
    this.currentGroupId = this.parentStack.length === 0 ? null : parent.id;
    this._init(parent);

    this.history.reset();
    this.execute();
    this.save();

    return { nodeId: parent.nodeId };
  }

  createNodeId() {
    const ids = [
      ...this.nodes.keys(),
      ...this.groups.map(g => g.id),
      ...this.groups.flatMap(g => g.nodes.map(n => n.id))
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

  getUnusedGroups() {
    const usedGroupIds = new SvelteSet<number>();
    const queue: number[] = [];

    // Seed from root-level nodes so outer groups aren't treated as unused when inside a group.
    const rootNodes = this.parentStack.length > 0
      ? this.parentStack[0].nodes
      : [...this.nodes.values()];

    for (const node of rootNodes) {
      if (node.type === '__internal/group/instance' && node.props?.groupId !== undefined) {
        queue.push(node.props.groupId as number);
      }
    }

    // Also seed from live nodes (may contain new group instances created inside a group).
    if (this.currentGroupId !== null) {
      for (const node of this.nodes.values()) {
        if (node.type === '__internal/group/instance' && node.props?.groupId !== undefined) {
          queue.push(node.props.groupId as number);
        }
      }
    }

    // Every group on the navigation path is used by definition.
    for (const entry of this.parentStack) {
      if (entry.id !== this.id) usedGroupIds.add(entry.id);
    }
    if (this.currentGroupId !== null) usedGroupIds.add(this.currentGroupId);

    while (queue.length) {
      const groupId = queue.pop()!;
      if (usedGroupIds.has(groupId)) continue;
      usedGroupIds.add(groupId);
      const group = this.getGroup(groupId);
      if (!group) continue;
      for (const node of group.nodes) {
        if (node.type === '__internal/group/instance' && node.props?.groupId !== undefined) {
          const childId = node.props.groupId as number;
          if (!usedGroupIds.has(childId)) queue.push(childId);
        }
      }
    }

    return this.groups.filter(g => !usedGroupIds.has(g.id));
  }

  removeUnusedGroups() {
    const unused = this.getUnusedGroups();
    const unusedIds = new SvelteSet(unused.map(g => g.id));
    this.groups = this.groups.filter(g => !unusedIds.has(g.id));
    this.save();
    return unused.length;
  }

  groupNodes(nodeIds: number[]) {
    this.startUndoGroup();
    this.removeUnusedGroups();

    const nodes = [
      ...new SvelteSet(nodeIds).values().map(id => this.getNode(id)).filter(Boolean)
    ] as NodeInstance[];

    if (!nodes.length) return;

    log.log(`Grouping ${nodes.length} nodes`, { nodes });

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

    const outputs = [];
    if (groupOutputs.size) {
      const edge = groupOutputs.values().next().value!;
      outputs.push({
        label: `Output`,
        type: edge[2].state.type?.inputs?.[edge[3]].type || '*'
      });
    }

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
      ...this.groups.map(g => g.id),
      ...this.groups.flatMap(g => g.nodes.map(n => n.id))
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
    this.groups.push(groupDefinition);

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

  ungroupNode(nodeId: number) {
    const groupNode = this.getNode(nodeId);
    if (!groupNode || groupNode.type !== '__internal/group/instance') return false;

    const groupId = groupNode.props?.groupId as number;
    const group = this.getGroup(groupId);
    if (!group) return false;

    this.startUndoGroup();

    const edgesToGroup = this.edges.filter(e => e[2].id === nodeId);
    const edgesFromGroup = this.edges.filter(e => e[0].id === nodeId);

    const groupInputNode = group.nodes.find(n => n.type === '__internal/group/input');
    const groupOutputNode = group.nodes.find(n => n.type === '__internal/group/output');
    const internalNodes = group.nodes.filter(
      n => n.type !== '__internal/group/input' && n.type !== '__internal/group/output'
    );

    // Offset internal nodes so their average position matches the group node's position
    let centerX = 0, centerY = 0;
    for (const n of internalNodes) {
      centerX += n.position[0];
      centerY += n.position[1];
    }
    const offsetX = internalNodes.length ? groupNode.position[0] - centerX / internalNodes.length : 0;
    const offsetY = internalNodes.length ? groupNode.position[1] - centerY / internalNodes.length : 0;

    // Allocate new IDs that don't collide with anything in the current graph
    const usedIds = new SvelteSet<number>([
      ...this.nodes.keys(),
      ...this.groups.map(g => g.id),
      ...this.groups.flatMap(g => g.nodes.map(n => n.id))
    ]);
    const nextId = () => {
      let id = 0;
      while (usedIds.has(id)) id++;
      usedIds.add(id);
      return id;
    };

    // Map old internal IDs (including boundary nodes) to fresh IDs
    const idMap = new Map<number, number>();
    for (const n of group.nodes) {
      idMap.set(n.id, nextId());
    }

    // Instantiate internal nodes and add them to the graph
    const newNodes: NodeInstance[] = internalNodes.map(n => {
      const nodeType = this.registry.getNode(n.type);
      const node: NodeInstance = $state({
        id: idMap.get(n.id)!,
        type: n.type,
        position: [n.position[0] + offsetX, n.position[1] + offsetY] as [number, number],
        state: { type: nodeType },
        props: n.props || {}
      });
      return node;
    });

    for (const node of newNodes) {
      this.nodes.set(node.id, node);
    }

    // input_X socket on the group node → the external source that was feeding it
    const inputIdxToExternal = new Map<number, { node: NodeInstance; socket: number }>();
    for (const edge of edgesToGroup) {
      const match = (edge[3] as string).match(/^input_(\d+)$/);
      if (match) inputIdxToExternal.set(parseInt(match[1]), { node: edge[0], socket: edge[1] });
    }

    // All external nodes that received output from the group node
    const externalOutputTargets = edgesFromGroup.map(e => ({ toNode: e[2], toSocket: e[3] }));

    // Recreate internal edges, substituting boundary nodes with the real external peers
    for (const [fromId, fromSocketIdx, toId, toSocketKey] of group.edges) {
      let fromNode: NodeInstance | undefined;
      let resolvedFromSocket = fromSocketIdx;

      if (groupInputNode && fromId === groupInputNode.id) {
        const ext = inputIdxToExternal.get(fromSocketIdx);
        if (!ext) continue;
        fromNode = ext.node;
        resolvedFromSocket = ext.socket;
      } else {
        const newId = idMap.get(fromId);
        if (newId !== undefined) fromNode = this.nodes.get(newId);
      }

      if (!fromNode) continue;

      if (groupOutputNode && toId === groupOutputNode.id) {
        for (const { toNode, toSocket } of externalOutputTargets) {
          this.createEdge(fromNode, resolvedFromSocket, toNode, toSocket, { applyUpdate: false });
        }
      } else {
        const newToId = idMap.get(toId);
        if (newToId === undefined) continue;
        const toNode = this.nodes.get(newToId);
        if (!toNode) continue;
        this.createEdge(fromNode, resolvedFromSocket, toNode, toSocketKey, { applyUpdate: false });
      }
    }

    // Remove the group instance node (also cleans up its edges)
    this.removeNode(groupNode);

    this.saveUndoGroup();

    return newNodes;
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
      log.error(`Node type not found: ${type}`);
      return;
    }

    const node: NodeInstance = $state({
      id: this.createNodeId(),
      type,
      position,
      state: { type: nodeType },
      props
    });

    log.log('creating node', { id: node.id, type, position, props });
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
      log.error('Edge already exists', existingEdge);
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
      log.error(
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

    log.log('creating edge', { from: from.id, fromSocket, to: to.id, toSocket });
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
    log.log('undo');
    const nextState = this.history.undo();
    if (nextState) {
      this._init(nextState);
      this.emit('save', this.serialize());
    }
  }

  redo() {
    log.log('redo');
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

    const fullState = this.serialize();
    this.emit('save', fullState);
    log.log('saving graphs', fullState);
  }

  getParentsOfNode(node: NodeInstance) {
    const parents = [];
    const stack = node.state?.parents?.slice(0);
    while (stack?.length) {
      if (parents.length > 1000000) {
        log.warn('Infinite loop detected');
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
