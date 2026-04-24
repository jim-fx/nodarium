import throttle from '$lib/helpers/throttle';
import { RemoteNodeRegistry } from '$lib/node-registry/index';
import type {
  Edge,
  Graph,
  GroupSocket,
  NodeDefinition,
  NodeGroupDefinition,
  NodeId,
  NodeInput,
  NodeInstance,
  NodeRegistry,
  SerializedNode,
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

function isVirtualType(type: string): boolean {
  return type.startsWith('__virtual/');
}

function isGroupInstanceType(type: string): boolean {
  return type === '__virtual/group/instance';
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

  // Group-related state
  groups = new SvelteMap<string, NodeGroupDefinition>();
  groupNodeDefinitions: Map<string, NodeDefinition> = new Map();
  currentGroupContext: string | null = null;
  graphStack: { rootGraph: Graph; groupId: string; nodeId: number; cameraPosition: [number, number, number] }[] = $state([]);

  inputSockets = $derived.by(() => {
    const s = new SvelteSet<string>();
    for (const edge of this.edges) {
      s.add(`${edge[2].id}-${edge[3]}`);
    }
    return s;
  });

  history: HistoryManager = new HistoryManager();
  private serializeFullGraph(): Graph {
    if (this.graphStack.length === 0) return this.serialize();
    // Merge the current internal state upward through every stack level.
    // $state.snapshot strips Svelte reactive proxies so the result can cross
    // the postMessage boundary to the worker.
    let merged: Graph = this.serialize();
    for (let i = this.graphStack.length - 1; i >= 0; i--) {
      const { rootGraph, groupId } = $state.snapshot(this.graphStack[i]);
      // Prefer the live definition (may have been updated via addGroupSocket/rename)
      // over the snapshot taken when we entered the group.
      const currentDef = $state.snapshot((this.graph.groups ?? rootGraph.groups)?.[groupId]);
      merged = {
        ...rootGraph,
        groups: {
          ...rootGraph.groups,
          [groupId]: {
            ...(currentDef as NodeGroupDefinition),
            graph: { nodes: merged.nodes, edges: merged.edges }
          }
        }
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

  // --- Group helpers ---

  private buildGroupNodeDefinition(group: NodeGroupDefinition): NodeDefinition {
    return {
      id: `__virtual/group/${group.id}` as NodeId,
      meta: { title: group.name },
      inputs: {
        // Placeholder for the group-selector dropdown — counted in height/socket math
        '__virtual/groupId': { type: 'select' as const, internal: true, label: '' } as NodeInput,
        ...Object.fromEntries(
          group.inputs.map(s => [s.name, { type: s.type, external: true } as NodeInput])
        )
      },
      outputs: group.outputs.map(s => s.type),
      execute(input: Int32Array): Int32Array { return input; }
    };
  }

  buildGroupInputNodeDef(group: NodeGroupDefinition): NodeDefinition {
    return {
      id: '__virtual/group/input' as NodeId,
      meta: { title: 'Input' },
      // Each group input socket gets a labeled row (external = no control widget,
      // internal = no left-side socket dot; it's an output-only node).
      inputs: Object.fromEntries(
        group.inputs.map(s => [s.name, { type: s.type, external: true, internal: true }])
      ) as Record<string, NodeInput>,
      outputs: group.inputs.map(s => s.type),
      execute(input: Int32Array): Int32Array { return input; }
    };
  }

  buildGroupOutputNodeDef(group: NodeGroupDefinition): NodeDefinition {
    return {
      id: '__virtual/group/output' as NodeId,
      inputs: Object.fromEntries(
        group.outputs.map(s => [s.name, { type: s.type }])
      ) as Record<string, NodeInput>,
      outputs: [],
      execute(input: Int32Array): Int32Array { return input; }
    };
  }

  private getNodeTypeWithContext(type: string, props?: Record<string, unknown>): NodeDefinition | undefined {
    if (type === '__virtual/group/input' && this.currentGroupContext) {
      const group = this.groups.get(this.currentGroupContext);
      if (group) return this.buildGroupInputNodeDef(group);
    }
    if (type === '__virtual/group/output' && this.currentGroupContext) {
      const group = this.groups.get(this.currentGroupContext);
      if (group) return this.buildGroupOutputNodeDef(group);
    }
    if (type === '__virtual/group/instance') {
      const groupId = props?.groupId as string | undefined;
      if (groupId) return this.groupNodeDefinitions.get(`__virtual/group/${groupId}`);
      return undefined;
    }
    return this.groupNodeDefinitions.get(type) || this.registry.getNode(type);
  }

  // --- Group creation ---

  createGroup(nodeIds: number[]): NodeInstance | undefined {
    if (nodeIds.length === 0) return;

    const selectedNodes = nodeIds
      .map(id => this.getNode(id))
      .filter(Boolean) as NodeInstance[];
    if (selectedNodes.length === 0) return;

    const selectedSet = new Set(nodeIds);

    // Snapshot boundary edges
    const incomingEdges = this.edges.filter(e =>
      !selectedSet.has(e[0].id) && selectedSet.has(e[2].id)
    );
    const outgoingEdges = this.edges.filter(e =>
      selectedSet.has(e[0].id) && !selectedSet.has(e[2].id)
    );

    const inputs: GroupSocket[] = incomingEdges.map((e, i) => ({
      name: `input_${i}`,
      type: e[0].state.type?.outputs?.[e[1]] || '*'
    }));

    const outputs: GroupSocket[] = outgoingEdges.map((e, i) => ({
      name: `output_${i}`,
      type: e[0].state.type?.outputs?.[e[1]] || '*'
    }));

    const groupId = `grp_${Date.now().toString(36)}`;

    const xs = selectedNodes.map(n => n.position[0]);
    const ys = selectedNodes.map(n => n.position[1]);
    const minX = Math.min(...xs);
    const maxX = Math.max(...xs);
    const avgY = ys.reduce((a, b) => a + b, 0) / ys.length;
    const centroidX = xs.reduce((a, b) => a + b, 0) / xs.length;

    // Find unique IDs for virtual nodes in the internal graph
    const existingIds = new Set(selectedNodes.map(n => n.id));
    let internalInputId = 1;
    while (existingIds.has(internalInputId)) internalInputId++;
    existingIds.add(internalInputId);
    let internalOutputId = internalInputId + 1;
    while (existingIds.has(internalOutputId)) internalOutputId++;

    const internalNodes: SerializedNode[] = [
      {
        id: internalInputId,
        type: '__virtual/group/input' as NodeId,
        position: [minX - 25, avgY]
      },
      ...selectedNodes.map(n => {
        // Use $state.snapshot to get plain values (no reactive proxies)
        const props = n.props ? $state.snapshot(n.props) : undefined;
        const meta = n.meta ? $state.snapshot(n.meta) : undefined;
        return {
          id: n.id,
          type: n.type,
          position: [n.position[0], n.position[1]] as [number, number],
          ...(props !== undefined ? { props } : {}),
          ...(meta ? { meta } : {})
        };
      }),
      {
        id: internalOutputId,
        type: '__virtual/group/output' as NodeId,
        position: [maxX + 25, avgY]
      }
    ];

    const internalEdges: Graph['edges'] = [
      ...this.getEdgesBetweenNodes(selectedNodes),
      ...incomingEdges.map((e, i) =>
        [internalInputId, i, e[2].id, e[3]] as [number, number, number, string]
      ),
      ...outgoingEdges.map((e, i) =>
        [e[0].id, e[1], internalOutputId, `output_${i}`] as [number, number, number, string]
      )
    ];

    const group: NodeGroupDefinition = {
      id: groupId,
      name: 'Group',
      inputs,
      outputs,
      graph: { nodes: internalNodes, edges: internalEdges }
    };

    this.groups.set(groupId, group);
    if (!this.graph.groups) this.graph.groups = {};
    this.graph.groups[groupId] = group;

    const groupNodeDef = this.buildGroupNodeDefinition(group);
    this.groupNodeDefinitions.set(groupNodeDef.id, groupNodeDef);

    this.startUndoGroup();

    // Remove selected nodes and all their edges
    for (const node of selectedNodes) {
      const connectedEdges = this.edges.filter(
        e => e[0].id === node.id || e[2].id === node.id
      );
      for (const e of connectedEdges) {
        this.removeEdge(e, { applyDeletion: false });
      }
      this.nodes.delete(node.id);
    }

    // Place group instance node (plain object like _init — don't wrap in $state()
    // to avoid Svelte 5 deeply-proxying the NodeDefinition execute function)
    const groupNodeId = this.createNodeId();
    const groupNode = {
      id: groupNodeId,
      type: '__virtual/group/instance' as NodeId,
      position: [centroidX, avgY] as [number, number],
      props: { groupId },
      state: { type: groupNodeDef }
    } as NodeInstance;
    this.nodes.set(groupNodeId, groupNode);

    // Reconnect boundary edges
    for (let i = 0; i < incomingEdges.length; i++) {
      const e = incomingEdges[i];
      this.createEdge(e[0], e[1], groupNode, inputs[i].name, { applyUpdate: false });
    }
    for (let i = 0; i < outgoingEdges.length; i++) {
      const e = outgoingEdges[i];
      this.createEdge(groupNode, i, e[2], e[3], { applyUpdate: false });
    }

    this.saveUndoGroup();
    this.execute();

    return groupNode;
  }

  // --- Ungrouping ---

  ungroup(nodeId: number) {
    const groupNode = this.getNode(nodeId);
    if (!groupNode || !isGroupInstanceType(groupNode.type)) return;

    const groupId = groupNode.props?.groupId as string | undefined;
    if (!groupId) return;
    const group = this.groups.get(groupId);
    if (!group) return;

    const incomingEdges = this.getEdgesToNode(groupNode);
    const outgoingEdges = this.getEdgesFromNode(groupNode);

    const inputVirtualId = group.graph.nodes.find(
      n => n.type === '__virtual/group/input'
    )?.id;
    const outputVirtualId = group.graph.nodes.find(
      n => n.type === '__virtual/group/output'
    )?.id;

    this.startUndoGroup();

    // Remove the group instance node (and its edges)
    this.removeNode(groupNode, { restoreEdges: false });

    // Re-insert internal nodes
    const idMap = new Map<number, number>();
    const realInternalNodes = group.graph.nodes.filter(
      n => n.type !== '__virtual/group/input' && n.type !== '__virtual/group/output'
    );

    for (const n of realInternalNodes) {
      const newId = this.createNodeId();
      idMap.set(n.id, newId);
      const nodeType = this.getNodeTypeWithContext(n.type, n.props as Record<string, unknown>);
      const newNode: NodeInstance = $state({
        id: newId,
        type: n.type,
        position: [...n.position] as [number, number],
        ...(n.props ? { props: { ...n.props } } : {}),
        state: nodeType ? { type: nodeType } : {}
      });
      this.nodes.set(newId, newNode);
    }

    // Re-wire edges
    for (const e of group.graph.edges) {
      const fromIsInput = e[0] === inputVirtualId;
      const toIsOutput = e[2] === outputVirtualId;

      if (fromIsInput) {
        const inputIdx = e[1];
        const parentEdge = incomingEdges.find(
          pe => pe[3] === group.inputs[inputIdx]?.name
        );
        if (parentEdge) {
          const toNode = this.getNode(idMap.get(e[2])!);
          if (toNode) {
            this.createEdge(parentEdge[0], parentEdge[1], toNode, e[3], {
              applyUpdate: false
            });
          }
        }
      } else if (toIsOutput) {
        const outputSocketName = e[3];
        const outputIdx = group.outputs.findIndex(s => s.name === outputSocketName);
        const parentEdge = outgoingEdges.find(pe => pe[1] === outputIdx);
        if (parentEdge) {
          const fromNode = this.getNode(idMap.get(e[0])!);
          const toNode = this.getNode(parentEdge[2].id);
          if (fromNode && toNode) {
            this.createEdge(fromNode, e[1], toNode, parentEdge[3], {
              applyUpdate: false
            });
          }
        }
      } else {
        const fromNode = this.getNode(idMap.get(e[0])!);
        const toNode = this.getNode(idMap.get(e[2])!);
        if (fromNode && toNode) {
          this.createEdge(fromNode, e[1], toNode, e[3], { applyUpdate: false });
        }
      }
    }

    // Remove group definition if no more instances
    const hasOtherInstances = Array.from(this.nodes.values()).some(
      n => n.type === '__virtual/group/instance' && (n.props?.groupId as string) === groupId
    );
    if (!hasOtherInstances) {
      this.groups.delete(groupId);
      this.groupNodeDefinitions.delete(`__virtual/group/${groupId}`);
      if (this.graph.groups) {
        delete this.graph.groups[groupId];
      }
    }

    this.saveUndoGroup();
    this.execute();
  }

  // --- Group socket management (called from inside a group) ---

  addGroupSocket(kind: 'input' | 'output', socketType: string) {
    if (!this.currentGroupContext) return;
    const group = this.groups.get(this.currentGroupContext);
    if (!group) return;

    const oldArr = kind === 'input' ? group.inputs : group.outputs;
    const name = `${kind}_${oldArr.length}`;
    const updatedGroup: NodeGroupDefinition = {
      ...group,
      inputs: kind === 'input' ? [...oldArr, { name, type: socketType }] : group.inputs,
      outputs: kind === 'output' ? [...oldArr, { name, type: socketType }] : group.outputs
    };

    if (!this.graph.groups) this.graph.groups = {};
    this.graph.groups[this.currentGroupContext] = updatedGroup;

    // Reinitialize so virtual group input/output nodes pick up the new socket reactively
    this._init(this.serialize());
    this.save();
  }

  removeGroupSocket(kind: 'input' | 'output', index: number) {
    if (!this.currentGroupContext) return;
    const group = this.groups.get(this.currentGroupContext);
    if (!group) return;

    const oldArr = kind === 'input' ? group.inputs : group.outputs;
    const updatedGroup: NodeGroupDefinition = {
      ...group,
      inputs: kind === 'input' ? oldArr.filter((_, i) => i !== index) : group.inputs,
      outputs: kind === 'output' ? oldArr.filter((_, i) => i !== index) : group.outputs
    };

    if (!this.graph.groups) this.graph.groups = {};
    this.graph.groups[this.currentGroupContext] = updatedGroup;

    this._init(this.serialize());
    this.save();
  }

  private _refreshGroupContext(group: NodeGroupDefinition) {
    const groupId = group.id;

    // Keep graph.groups in sync
    if (this.graph.groups?.[groupId]) {
      this.graph.groups[groupId] = group;
    }

    // Rebuild the group node definition (used in parent graph)
    const groupNodeDef = this.buildGroupNodeDefinition(group);
    this.groupNodeDefinitions.set(groupNodeDef.id, groupNodeDef);

    // Update virtual input/output nodes in the current internal graph,
    // and any group instance nodes that reference this group
    const inputDef = this.buildGroupInputNodeDef(group);
    const outputDef = this.buildGroupOutputNodeDef(group);
    for (const node of this.nodes.values()) {
      if (node.type === '__virtual/group/input') node.state.type = inputDef;
      if (node.type === '__virtual/group/output') node.state.type = outputDef;
      if (node.type === '__virtual/group/instance' && (node.props?.groupId as string) === groupId) {
        node.state.type = groupNodeDef;
      }
    }
  }

  pruneUnusedGroups() {
    const usedIds = new Set<string>();

    // Scan nodes in the current graph
    for (const node of this.nodes.values()) {
      if (node.type === '__virtual/group/instance') {
        const gid = node.props?.groupId as string | undefined;
        if (gid) usedIds.add(gid);
      }
    }

    // Scan nodes in every stacked (parent) graph
    for (const { rootGraph } of this.graphStack) {
      for (const node of rootGraph.nodes) {
        if (node.type === '__virtual/group/instance' && node.props?.groupId) {
          usedIds.add(node.props.groupId as string);
        }
      }
    }

    // Scan internal graphs of all known groups (nested group nodes)
    for (const group of this.groups.values()) {
      for (const node of group.graph.nodes) {
        if (node.type === '__virtual/group/instance' && node.props?.groupId) {
          usedIds.add(node.props.groupId as string);
        }
      }
    }

    let changed = false;
    for (const groupId of [...this.groups.keys()]) {
      if (!usedIds.has(groupId)) {
        this.groups.delete(groupId);
        this.groupNodeDefinitions.delete(`__virtual/group/${groupId}`);
        if (this.graph.groups) delete this.graph.groups[groupId];
        changed = true;
      }
    }

    if (changed) {
      this.execute();
      this.save();
    }
  }

  // --- Group navigation ---

  enterGroup(nodeId: number, cameraPosition: [number, number, number] = [0, 0, 4]): boolean {
    const groupNode = this.getNode(nodeId);
    if (!groupNode || !isGroupInstanceType(groupNode.type)) return false;

    const groupId = groupNode.props?.groupId as string | undefined;
    if (!groupId) return false;
    const group = this.groups.get(groupId);
    if (!group) return false;

    const currentSerialized = this.serialize();
    this.graphStack.push({ rootGraph: currentSerialized, groupId, nodeId, cameraPosition });

    this.currentGroupContext = groupId;

    const internalGraph: Graph = {
      id: this.graph.id,
      nodes: group.graph.nodes,
      edges: group.graph.edges,
      groups: this.graph.groups
    };

    this.graph = internalGraph;
    this._init(internalGraph);
    this.history.reset();

    return true;
  }

  exitGroup(): { camera: [number, number, number]; nodeId: number } | false {
    if (this.graphStack.length === 0) return false;

    const { rootGraph, groupId, nodeId, cameraPosition } = this.graphStack[this.graphStack.length - 1];
    this.graphStack.pop();

    // Serialize current internal graph state
    const internalState = this.serialize();

    // Update the group definition in the root graph
    const updatedRootGraph: Graph = {
      ...rootGraph,
      groups: {
        ...rootGraph.groups,
        [groupId]: {
          ...rootGraph.groups?.[groupId]!,
          graph: {
            nodes: internalState.nodes,
            edges: internalState.edges
          }
        }
      }
    };

    this.currentGroupContext = this.graphStack.length > 0
      ? this.graphStack[this.graphStack.length - 1].groupId
      : null;

    this.graph = updatedRootGraph;
    this._init(updatedRootGraph);
    this.history.reset();
    this.save();

    return { camera: cameraPosition, nodeId };
  }

  get isInsideGroup(): boolean {
    return this.graphStack.length > 0;
  }

  get breadcrumbs(): { name: string; groupId: string | null }[] {
    const crumbs: { name: string; groupId: string | null }[] = [
      { name: 'Root', groupId: null }
    ];
    for (const entry of this.graphStack) {
      const group = this.groups.get(entry.groupId);
      crumbs.push({ name: group?.name ?? entry.groupId, groupId: entry.groupId });
    }
    return crumbs;
  }

  // --- Serialization ---

  private serializeGroups(): Graph['groups'] | undefined {
    const src = this.graph.groups;
    if (!src || Object.keys(src).length === 0) return undefined;
    const result: NonNullable<Graph['groups']> = {};
    for (const [id, group] of Object.entries(src)) {
      result[id] = {
        id: group.id,
        name: group.name,
        inputs: group.inputs.map(s => ({ name: s.name, type: s.type })),
        outputs: group.outputs.map(s => ({ name: s.name, type: s.type })),
        graph: {
          nodes: group.graph.nodes.map(n => ({
            id: n.id,
            type: n.type,
            position: [n.position[0], n.position[1]] as [number, number],
            ...(n.props !== undefined ? {
              props: Object.fromEntries(
                Object.entries(n.props).map(([k, v]) => [
                  k,
                  Array.isArray(v) ? [...v] : v
                ])
              )
            } : {}),
            ...(n.meta ? { meta: { title: n.meta.title, lastModified: n.meta.lastModified } } : {})
          })),
          edges: group.graph.edges.map(
            e => [e[0], e[1], e[2], e[3]] as [number, number, number, string]
          )
        }
      };
    }
    return result;
  }

  serialize(): Graph {
    const nodes = Array.from(this.nodes.values()).map((node) => ({
      id: node.id,
      position: [...node.position] as [number, number],
      type: node.type,
      props: node.props ? $state.snapshot(node.props) : undefined
    }));
    const edges = this.edges.map((edge) => [
      edge[0].id,
      edge[1],
      edge[2].id,
      edge[3]
    ]) as Graph['edges'];

    const groups = this.serializeGroups();

    const serialized = {
      id: this.graph.id,
      settings: $state.snapshot(this.settings),
      meta: $state.snapshot(this.graph.meta),
      nodes,
      edges,
      ...(groups ? { groups } : {})
    };
    logger.log('serializing graph', serialized);
    return clone(serialized) as Graph;
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
    const all = this.registry.getAllNodes();
    // Only show the Group node in AddMenu when there's at least one group to assign
    if (this.groups.size === 0) {
      return all.filter(n => n.id !== '__virtual/group/instance');
    }
    return all;
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

    const parentIds = new SvelteSet(this.getParentsOfNode(draggedNode).map(n => n.id));

    return this.edges.filter((edge) => {
      const [fromNode, fromSocketIdx, toNode, toSocketKey] = edge;

      if (parentIds.has(toNode.id)) return false;
      if (fromNode.id === nodeId || toNode.id === nodeId) return false;

      const edgeOutputSocketType = fromNode.state?.type?.outputs?.[fromSocketIdx];
      const canPlugIntoDragged = draggedInputs.some(input => {
        const acceptedTypes = [input.type, ...(input.accepts || [])];
        return areSocketsCompatible(edgeOutputSocketType, acceptedTypes);
      });

      if (!canPlugIntoDragged) return false;

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
    // Rebuild group definitions from the graph
    this.groups.clear();
    this.groupNodeDefinitions.clear();
    if (graph.groups) {
      for (const [groupId, group] of Object.entries(graph.groups)) {
        this.groups.set(groupId, group);
        const def = this.buildGroupNodeDefinition(group);
        this.groupNodeDefinitions.set(def.id, def);
      }
    }

    const nodes = new SvelteMap(
      graph.nodes.map((serialized) => {
        // Migration: old __virtual/group/{groupId} format → __virtual/group/instance with props.groupId
        let node = serialized;
        if (node.type.startsWith('__virtual/group/')
            && node.type !== '__virtual/group/input'
            && node.type !== '__virtual/group/output'
            && node.type !== '__virtual/group/instance') {
          const oldGroupId = node.type.split('/')[2];
          node = { ...node, type: '__virtual/group/instance' as NodeId, props: { ...node.props, groupId: oldGroupId } };
        }

        // IMPORTANT: copy the node so we don't mutate the original SerializedNode
        // (which may be stored in a group definition). Mutating it would add
        // state.type (with an execute fn) making it non-cloneable.
        const nodeType = this.getNodeTypeWithContext(node.type, node.props as Record<string, unknown>);
        const n = { ...node } as NodeInstance;
        n.state = nodeType ? { type: nodeType } : {};
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
    this.graph = graph;
    this.status = 'loading';
    this.id = graph.id;

    logger.info('loading graph', { nodes: graph.nodes, edges: graph.edges, id: graph.id });

    // Filter out virtual group types — they are resolved locally, not fetched remotely
    const nodeIds = Array.from(new SvelteSet([
      ...graph.nodes.map((n) => n.type).filter(t => !isVirtualType(t))
    ]));
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
      if (isVirtualType(node.type)) continue;
      const nodeType = this.registry.getNode(node.type);
      if (!nodeType) {
        logger.error(`Node type not found: ${node.type}`);
        this.status = 'error';
        return;
      }
      const n = node as NodeInstance;
      n.state = {};
      n.state.type = nodeType;
    }

    // load settings
    const settingTypes: Record<
      string,
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

    // Reset navigation
    this.graphStack = [];
    this.currentGroupContext = null;

    this.history.reset();
    this._init(this.graph);

    this.save();

    this.status = 'idle';

    this.loaded = true;
    logger.log(`Graph loaded in ${performance.now() - a}ms`);
    setTimeout(() => this.execute(), 100);
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
    const toParents = this.getParentsOfNode(to);
    const fromParents = this.getParentsOfNode(from);
    if (toParents.includes(from)) {
      const fromChildren = this.getChildren(from);
      return toParents.filter((n) => fromChildren.includes(n));
    } else if (fromParents.includes(to)) {
      const toChildren = this.getChildren(to);
      return fromParents.filter((n) => toChildren.includes(n));
    } else {
      return;
    }
  }

  removeNode(node: NodeInstance, { restoreEdges = false } = {}) {
    if (node.type === '__virtual/group/input' || node.type === '__virtual/group/output') return;
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

  createNodeId() {
    return Math.max(0, ...this.nodes.keys()) + 1;
  }

  createGraph(nodes: NodeInstance[], edges: [number, number, number, string][]) {
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
    if (type === '__virtual/group/instance') {
      const firstEntry = this.groups.entries().next();
      if (firstEntry.done) {
        logger.error('No groups available to create a group node');
        return;
      }
      const [groupId] = firstEntry.value;
      const groupNodeDef = this.groupNodeDefinitions.get(`__virtual/group/${groupId}`);
      const node = {
        id: this.createNodeId(),
        type: '__virtual/group/instance' as NodeId,
        position,
        props: { groupId, ...props },
        state: { type: groupNodeDef }
      } as NodeInstance;
      this.nodes.set(node.id, node);
      this.save();
      return node;
    }

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

    const existingEdge = existingEdges.find(
      (e) => e[0].id === from.id && e[1] === fromSocket && e[3] === toSocket
    );
    if (existingEdge) {
      logger.error('Edge already exists', existingEdge);
      return;
    }

    const fromSocketType = from.state?.type?.outputs?.[fromSocket];
    const toSocketType = [to.state?.type?.inputs?.[toSocket]?.type];
    if (to.state?.type?.inputs?.[toSocket]?.accepts) {
      toSocketType.push(...(to?.state?.type?.inputs?.[toSocket]?.accepts || []));
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
    const nodeType = node?.state?.type;
    if (!nodeType) return [];

    const sockets: [NodeInstance, string | number][] = [];

    if (typeof index === 'string') {
      const children = new SvelteSet(this.getChildren(node).map((n) => n.id));
      const nodes = this.getAllNodes().filter(
        (n) => n.id !== node.id && !children.has(n.id)
      );

      const ownType = nodeType?.inputs?.[index].type;

      for (const node of nodes) {
        const nodeType = node?.state?.type;
        const inputs = nodeType?.outputs;
        if (!inputs) continue;
        for (let index = 0; index < inputs.length; index++) {
          if (inputs[index] === ownType) {
            sockets.push([node, index]);
          }
        }
      }
    } else if (typeof index === 'number') {
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
        const inputs = node?.state?.type?.inputs;
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
