import type {
  Frame,
  Graph,
  GroupDefinition,
  NodeDefinition,
  NodeInput,
  SerializedEdge,
  SerializedNode
} from '@nodarium/types';
import {
  DEBUG_NODE,
  getOutputType,
  GROUP_INPUT,
  GROUP_INSTANCE,
  GROUP_OUTPUT,
  groupIdOf,
  inputAccepts,
  INTERNAL_PREFIX,
  isOutputNode
} from '@nodarium/types';
import { debugNode } from './debug-node';
import { CURRENT_GRAPH_VERSION, migrateGraph } from './graph-migrations';

export type GraphIssueCode =
  | 'unknown-node-type'
  | 'unknown-input'
  | 'unknown-prop'
  | 'incompatible-edge'
  | 'missing-node'
  | 'output-out-of-range'
  | 'duplicate-id'
  | 'input-connected-twice'
  | 'no-output'
  | 'several-outputs'
  | 'cycle'
  | 'value-out-of-range'
  | 'wrong-value-type'
  | 'unknown-setting'
  | 'no-version'
  | 'unreachable-node'
  | 'frame-missing-node'
  | 'node-in-two-frames'
  | 'duplicate-frame-id';

/** A mistake in a graph, found by {@link validateGraph} */
export type GraphIssue = {
  /** a graph with an error doesn't build what it says, one with a warning does */
  severity: 'error' | 'warning';
  code: GraphIssueCode;
  /** like `noise#2 has no input "strenght", did you mean "strength"?` */
  message: string;
  /** the node the issue is about, inside the group `groupId` if that is set */
  nodeId?: number;
  /** the group the issue is in, missing for the graph itself */
  groupId?: number;
  /** the frame the issue is about, of the graph or of the group `groupId` */
  frameId?: number;
  edge?: SerializedEdge;
  /** what to write instead: a name, a node type or a value */
  suggestion?: string;
};

const OUTPUT = 'max/core/output';
const TO_GEOMETRY = 'max/plantarium/to_geometry';

const WARNINGS = new Set<GraphIssueCode>([
  'value-out-of-range',
  'unknown-setting',
  'unreachable-node',
  // frames compute nothing, the editor drops what is wrong with them
  'frame-missing-node',
  'node-in-two-frames',
  'duplicate-frame-id'
]);

/** The inputs and output types of a node as the runtime sees them */
type Sockets = {
  inputs: Record<string, NodeInput>;
  outputs: (string | undefined)[];
};

/** The graph itself or one of its groups */
type Scope = {
  nodes: SerializedNode[];
  edges: SerializedEdge[];
  frames?: Frame[];
  group?: GroupDefinition;
};

type Details = Pick<GraphIssue, 'nodeId' | 'groupId' | 'frameId' | 'edge' | 'suggestion'>;

type Report = (code: GraphIssueCode, message: string, details?: Details) => void;

function list<T>(value: T[] | undefined): T[] {
  return Array.isArray(value) ? value : [];
}

function editDistance(a: string, b: string) {
  let previous = Array.from({ length: b.length + 1 }, (_, i) => i);
  for (let i = 1; i <= a.length; i++) {
    const current = [i];
    for (let j = 1; j <= b.length; j++) {
      current[j] = Math.min(
        previous[j] + 1,
        current[j - 1] + 1,
        previous[j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1)
      );
    }
    previous = current;
  }
  return previous[b.length];
}

/**
 * The candidates closest to `name`, the first of them with the smallest edit distance. Those that
 * differ in more than a third are left out. `short` compares only a part of the names.
 */
function nearest(
  name: string,
  candidates: Iterable<string>,
  short: (name: string) => string = n => n
) {
  const target = short(name).toLowerCase();
  let best = Infinity;
  let closest: string[] = [];
  for (const candidate of candidates) {
    const other = short(candidate).toLowerCase();
    const distance = editDistance(target, other);
    if (distance > Math.max(2, Math.floor(Math.max(target.length, other.length) / 3))) continue;
    if (distance < best) [best, closest] = [distance, []];
    if (distance === best) closest.push(candidate);
  }
  return closest;
}

/** The nodes of the same collection first, then the others */
function nearestTypes(type: string, candidates: string[]) {
  const collection = (id: string) => id.split('/').slice(0, 2).join('/');
  const close = nearest(type, candidates, id => id.split('/').pop() ?? id);
  return close.sort((a, b) =>
    Number(collection(b) === collection(type)) - Number(collection(a) === collection(type))
  ).slice(0, 3);
}

function didYouMean(names: string[]) {
  if (names.length === 0) return '';
  const quoted = names.map(n => `"${n}"`);
  const last = quoted.pop()!;
  return `, did you mean ${quoted.length ? `${quoted.join(', ')} or ${last}` : last}?`;
}

const SHORT_NAMES: Record<string, string> = {
  [GROUP_INSTANCE]: 'group',
  [GROUP_INPUT]: 'group_input',
  [GROUP_OUTPUT]: 'group_output',
  [DEBUG_NODE]: 'debug'
};

function nodeName(node: { id: number; type: string }) {
  const type = typeof node.type === 'string' ? node.type : '';
  return `${SHORT_NAMES[type] ?? type.split('/').pop()}#${node.id}`;
}

function groupName(group: GroupDefinition) {
  return group.name ? `group "${group.name}"` : `group ${group.id}`;
}

function describeTypes(input: NodeInput) {
  const types = [input.type, ...(input.accepts ?? [])];
  return types.length > 1 ? `${types.slice(0, -1).join(', ')} or ${types.at(-1)}` : input.type;
}

function isNumberArray(value: unknown) {
  return Array.isArray(value) && value.every(v => typeof v === 'number');
}

/** Why `value` can't be the value of `input`, undefined if it can */
function wrongValueType(input: NodeInput, value: unknown): string | undefined {
  switch (input.type) {
    case 'float':
    case 'integer':
      return typeof value === 'number' && Number.isFinite(value) ? undefined : 'a number';
    case 'boolean':
      return typeof value === 'boolean' || value === 0 || value === 1
        ? undefined
        : 'true, false, 1 or 0';
    case 'select': {
      const options = (input.options ?? []).map((o, i) =>
        typeof o === 'string' ? `${i} "${o}"` : `${o.value} "${o.label}"`
      );
      const names = (input.options ?? []).map(o => (typeof o === 'string' ? o : o.label));
      if (typeof value === 'number' || (typeof value === 'string' && names.includes(value))) {
        return undefined;
      }
      return options.length
        ? `the number or the name of an option: ${options.join(', ')}`
        : 'the index or the name of an option';
    }
    case 'vec3':
      return isNumberArray(value) && (value as number[]).length === 3
        ? undefined
        : 'three numbers';
    case '*':
      return undefined;
    default:
      return isNumberArray(value) ? undefined : 'a list of numbers';
  }
}

/**
 * The editor treats min and max as the range of the slider, a value can be typed or dragged
 * past them. Only a value that lies further outside than this many times the width of the
 * range is taken for a mistake, like a strength of 99999 for a range of 0 to 1.
 */
const RANGE_SLACK = 100;

/** Why `value` lies outside of what `input` takes, undefined if it doesn't */
function outOfRange(input: NodeInput, value: unknown): string | undefined {
  if ((input.type === 'float' || input.type === 'integer') && typeof value === 'number') {
    const { min, max } = input;
    const width = min !== undefined && max !== undefined && max > min
      ? max - min
      : Math.max(Math.abs(min ?? 0), Math.abs(max ?? 0), 1);
    const range = `${min ?? '-∞'} to ${max ?? '∞'}`;
    if (min !== undefined && value < min - RANGE_SLACK * width) {
      return `far below the range ${range} of the input`;
    }
    if (max !== undefined && value > max + RANGE_SLACK * width) {
      return `far above the range ${range} of the input`;
    }
  }
  if (input.type === 'select' && input.options?.length) {
    const names = input.options.map(o => (typeof o === 'string' ? o : o.label));
    if (typeof value === 'string' && !names.includes(value)) {
      return `not one of the options ${names.map(n => `"${n}"`).join(', ')}`;
    }
    if (typeof value === 'number' && (value < 0 || value >= names.length)) {
      return `the input has ${names.length} options, 0 to ${names.length - 1}`;
    }
  }
  return undefined;
}

/** Inputs that take their value from somewhere else than an edge or a prop */
function readsElsewhere(input: NodeInput) {
  if (input.setting) return `takes its value from the setting "${input.setting}"`;
  if (input.type === 'seed') return 'takes the seed of the graph';
  if (input.type === 'creator') return 'takes the id of the node';
  return undefined;
}

function groupInstanceSockets(group: GroupDefinition): Sockets {
  return {
    inputs: {
      groupId: { type: 'select', internal: true },
      ...group.inputs
    },
    outputs: list(group.outputs).map(o => o.type)
  };
}

function groupInputSockets(group: GroupDefinition): Sockets {
  return { inputs: {}, outputs: Object.values(group.inputs ?? {}).map(i => i.type) };
}

function groupOutputSockets(group: GroupDefinition): Sockets {
  const outputs = list(group.outputs);
  const inputs: Record<string, NodeInput> = Object.fromEntries(
    outputs.map((o, i) => [`out_${i}`, { type: o.type } as NodeInput])
  );
  // older groups named their single output `Out`
  if (outputs.length) inputs.Out = inputs.out_0;
  return { inputs, outputs: [] };
}

function definitionSockets(definition: NodeDefinition): Sockets {
  return {
    inputs: definition.inputs ?? {},
    outputs: (definition.outputs ?? []).map(getOutputType)
  };
}

class GraphValidator {
  readonly issues: GraphIssue[] = [];
  private readonly groups = new Map<number, GroupDefinition>();
  /** the settings the nodes of the graph read */
  private readonly readSettings = new Set(['seed']);

  constructor(
    private readonly graph: Graph,
    private readonly definitions: Map<string, NodeDefinition>
  ) {}

  run() {
    const groups = list(this.graph.groups);
    const groupScopes: Scope[] = [];
    for (const group of groups) {
      if (this.groups.has(group.id)) {
        this.issue('duplicate-id', `two groups have the id ${group.id}, only the first is used`, {
          groupId: group.id
        });
        continue;
      }
      this.groups.set(group.id, group);
      groupScopes.push({
        nodes: list(group.nodes),
        edges: list(group.edges),
        frames: list(group.frames),
        group
      });
    }

    this.checkGroupCycles();
    const root: Scope = {
      nodes: list(this.graph.nodes),
      edges: list(this.graph.edges),
      frames: list(this.graph.frames)
    };
    for (const scope of [root, ...groupScopes]) this.checkScope(scope);
    this.checkOutputs(root);
    this.checkSettings();
    return this.issues;
  }

  /** An issue inside `group` names the group after the message */
  private issue(
    code: GraphIssueCode,
    message: string,
    details: Details = {},
    group?: GroupDefinition
  ) {
    this.issues.push({
      severity: WARNINGS.has(code) ? 'warning' : 'error',
      code,
      message: group ? `${message} (in ${groupName(group)})` : message,
      ...details,
      ...(group ? { groupId: group.id } : {})
    });
  }

  private socketsOf(node: SerializedNode, scope: Scope, report: Report): Sockets | undefined {
    const type = node.type;
    if (type === GROUP_INSTANCE) {
      const groupId = groupIdOf(node);
      const group = groupId === undefined ? undefined : this.groups.get(groupId);
      if (!group) {
        const known = [...this.groups.keys()].join(', ') || 'none';
        report(
          'unknown-node-type',
          `${
            nodeName(node)
          } uses the group ${groupId}, the graph has no such group (groups: ${known})`,
          { nodeId: node.id }
        );
        return undefined;
      }
      return groupInstanceSockets(group);
    }
    if (type === GROUP_INPUT || type === GROUP_OUTPUT) {
      if (!scope.group) {
        report('unknown-node-type', `${nodeName(node)} only works inside a group`, {
          nodeId: node.id
        });
        return undefined;
      }
      return type === GROUP_INPUT
        ? groupInputSockets(scope.group)
        : groupOutputSockets(scope.group);
    }

    const definition = this.definitions.get(type)
      ?? (type === DEBUG_NODE ? debugNode as unknown as NodeDefinition : undefined);
    if (!definition) {
      const candidates = [...this.definitions.keys()].filter(id => !id.startsWith(INTERNAL_PREFIX));
      const close = typeof type === 'string' ? nearestTypes(type, candidates) : [];
      report(
        'unknown-node-type',
        `${nodeName(node)} has the unknown type "${type}"${didYouMean(close)}`,
        { nodeId: node.id, suggestion: close[0] }
      );
      return undefined;
    }
    return definitionSockets(definition);
  }

  private checkScope(scope: Scope) {
    const report: Report = (code, message, details) =>
      this.issue(code, message, details, scope.group);

    const connected = new Set<string>();
    for (const edge of scope.edges) if (Array.isArray(edge)) connected.add(`${edge[2]}/${edge[3]}`);

    const byId = new Map<number, SerializedNode>();
    const sockets = new Map<number, Sockets | undefined>();
    for (const node of scope.nodes) {
      const existing = byId.get(node.id);
      if (existing) {
        report(
          'duplicate-id',
          `${nodeName(existing)} and ${
            nodeName(node)
          } have the same id, every node needs an id of its own`,
          { nodeId: node.id }
        );
        continue;
      }
      byId.set(node.id, node);
      const nodeSockets = this.socketsOf(node, scope, report);
      sockets.set(node.id, nodeSockets);
      if (nodeSockets) this.checkNode(node, nodeSockets, connected, report);
    }

    this.checkEdges(scope, byId, sockets, report);
    this.checkCycles(scope, byId, report);
    this.checkReach(scope, byId, report);
    this.checkFrames(scope, byId, report);
  }

  private checkFrames(scope: Scope, byId: Map<number, SerializedNode>, report: Report) {
    const frameIds = new Set<number>();
    const frameOf = new Map<number, Frame>();
    for (const frame of list(scope.frames)) {
      const name = `the frame "${frame.label}"`;
      if (frameIds.has(frame.id)) {
        report('duplicate-frame-id', `two frames have the id ${frame.id}`, { frameId: frame.id });
      }
      frameIds.add(frame.id);
      for (const nodeId of list(frame.nodes)) {
        if (!byId.has(nodeId)) {
          report('frame-missing-node', `${name} names the node ${nodeId}, there is no such node`, {
            frameId: frame.id,
            nodeId
          });
          continue;
        }
        const other = frameOf.get(nodeId);
        if (other) {
          report(
            'node-in-two-frames',
            `${nodeName(byId.get(nodeId)!)} is in ${name} and in the frame "${other.label}", `
              + 'a node is in one frame at most, the first one keeps it',
            { frameId: frame.id, nodeId }
          );
          continue;
        }
        frameOf.set(nodeId, frame);
      }
    }
  }

  /** @param connected the inputs with an edge, as `<node id>/<input>` */
  private checkNode(
    node: SerializedNode,
    sockets: Sockets,
    connected: Set<string>,
    report: Report
  ) {
    for (const input of Object.values(sockets.inputs)) {
      if (input.setting) this.readSettings.add(input.setting);
    }

    const props = node.props && typeof node.props === 'object' ? node.props : {};
    for (const [key, value] of Object.entries(props)) {
      const input = sockets.inputs[key];
      const name = `${nodeName(node)} "${key}"`;
      if (!input) {
        const close = nearest(key, Object.keys(sockets.inputs));
        report(
          'unknown-prop',
          `${nodeName(node)} has no input "${key}"${didYouMean(close)}`,
          { nodeId: node.id, suggestion: close[0] }
        );
        continue;
      }
      const elsewhere = readsElsewhere(input);
      if (elsewhere) {
        report('unknown-prop', `${name} ${elsewhere}, the prop is never read, remove it`, {
          nodeId: node.id
        });
        continue;
      }
      // the editor keeps the value of an input it connects, the edge wins
      if (connected.has(`${node.id}/${key}`)) continue;
      const expected = wrongValueType(input, value);
      if (expected) {
        report('wrong-value-type', `${name} is ${JSON.stringify(value)}, it takes ${expected}`, {
          nodeId: node.id
        });
        continue;
      }
      const range = outOfRange(input, value);
      if (range) {
        report('value-out-of-range', `${name} is ${JSON.stringify(value)}, ${range}`, {
          nodeId: node.id
        });
      }
    }

    for (const key of list(node.exposed)) {
      if (sockets.inputs[key]) continue;
      const close = nearest(key, Object.keys(sockets.inputs));
      report(
        'unknown-input',
        `${nodeName(node)} shows the input "${key}", it has no such input${didYouMean(close)}`,
        { nodeId: node.id, suggestion: close[0] }
      );
    }
  }

  private checkEdges(
    scope: Scope,
    byId: Map<number, SerializedNode>,
    sockets: Map<number, Sockets | undefined>,
    report: Report
  ) {
    const connected = new Map<string, SerializedEdge>();
    for (const edge of scope.edges) {
      if (!Array.isArray(edge)) continue;
      const [fromId, output, toId, key] = edge;
      const from = byId.get(fromId);
      const to = byId.get(toId);
      if (!to) {
        const source = from ? nodeName(from) : `node ${fromId}`;
        report(
          'missing-node',
          `the edge from ${source} goes to node ${toId}, there is no node ${toId}`,
          {
            edge
          }
        );
        continue;
      }
      const target = `${nodeName(to)} "${key}"`;
      if (!from) {
        report(
          'missing-node',
          `the edge into ${target} comes from node ${fromId}, there is no node ${fromId}`,
          { nodeId: to.id, edge }
        );
        continue;
      }

      const twice = connected.get(`${toId}/${key}`);
      if (twice) {
        report(
          'input-connected-twice',
          `${target} is connected twice, from ${nodeName(byId.get(twice[0])!)} and ${
            nodeName(from)
          }, only the last edge counts`,
          { nodeId: to.id, edge }
        );
      }
      connected.set(`${toId}/${key}`, edge);

      const toSockets = sockets.get(toId);
      const fromSockets = sockets.get(fromId);
      const input = toSockets?.inputs[key];
      if (toSockets && !input) {
        const close = nearest(key, Object.keys(toSockets.inputs));
        report(
          'unknown-input',
          `${nodeName(to)} has no input "${key}"${didYouMean(close)}`,
          { nodeId: to.id, edge, suggestion: close[0] }
        );
      }
      const outputCount = fromSockets?.outputs.length ?? 0;
      const outputExists = Number.isInteger(output) && output >= 0 && output < outputCount;
      if (fromSockets && !outputExists) {
        const has = outputCount === 0
          ? 'has no outputs'
          : outputCount === 1
          ? 'only has output 0'
          : `has the outputs 0 to ${outputCount - 1}`;
        report(
          'output-out-of-range',
          `the edge into ${target} reads output ${output} of ${nodeName(from)}, which ${has}`,
          { nodeId: to.id, edge, suggestion: outputCount === 1 ? '0' : undefined }
        );
      }
      if (!input || !fromSockets || !outputExists) continue;

      const elsewhere = readsElsewhere(input);
      if (input.internal || elsewhere) {
        report(
          'incompatible-edge',
          `${target} ${elsewhere ?? 'can only be set on the node'}, it can't be connected`,
          { nodeId: to.id, edge }
        );
        continue;
      }
      const outputType = fromSockets.outputs[output];
      if (!inputAccepts(input, outputType)) {
        // paths and what sits on them become geometry for the output in a to_geometry
        const toGeometry = isRootOutput(to.type) && outputType === 'path';
        report(
          'incompatible-edge',
          `${target} takes ${describeTypes(input)}, output ${output} of ${
            nodeName(from)
          } is ${outputType}${toGeometry ? `, add a ${TO_GEOMETRY} between them` : ''}`,
          { nodeId: to.id, edge, suggestion: toGeometry ? TO_GEOMETRY : undefined }
        );
      }
    }
  }

  /** Nodes that depend on each other in a circle, each circle once */
  private checkCycles(scope: Scope, byId: Map<number, SerializedNode>, report: Report) {
    const children = new Map<number, number[]>();
    for (const edge of scope.edges) {
      if (!Array.isArray(edge) || !byId.has(edge[0]) || !byId.has(edge[2])) continue;
      children.set(edge[0], [...(children.get(edge[0]) ?? []), edge[2]]);
    }

    const state = new Map<number, 'open' | 'done'>();
    const path: number[] = [];
    const inCycle = new Set<number>();
    const visit = (id: number) => {
      state.set(id, 'open');
      path.push(id);
      for (const child of children.get(id) ?? []) {
        const childState = state.get(child);
        if (childState === 'open') {
          const cycle = path.slice(path.indexOf(child));
          if (cycle.some(n => inCycle.has(n))) continue;
          cycle.forEach(n => inCycle.add(n));
          const names = [...cycle, child].map(n => nodeName(byId.get(n)!));
          report('cycle', `the nodes depend on each other in a circle: ${names.join(' → ')}`, {
            nodeId: child
          });
        } else if (!childState) {
          visit(child);
        }
      }
      path.pop();
      state.set(id, 'done');
    };
    for (const id of byId.keys()) if (!state.has(id)) visit(id);
  }

  /** Nodes the output never reads from, nor a debug node, don't run */
  private checkReach(scope: Scope, byId: Map<number, SerializedNode>, report: Report) {
    const isSink = (node: SerializedNode) =>
      scope.group ? node.type === GROUP_OUTPUT : isRootOutput(node.type);
    const sinks = [...byId.values()].filter(isSink);
    if (sinks.length === 0) return;

    const parents = new Map<number, number[]>();
    for (const edge of scope.edges) {
      if (!Array.isArray(edge)) continue;
      parents.set(edge[2], [...(parents.get(edge[2]) ?? []), edge[0]]);
    }
    const reached = new Set<number>();
    const stack = [...byId.values()]
      .filter(node => isSink(node) || node.type === DEBUG_NODE)
      .map(node => node.id);
    while (stack.length) {
      const id = stack.pop()!;
      if (reached.has(id)) continue;
      reached.add(id);
      stack.push(...(parents.get(id) ?? []));
    }

    const target = scope.group ? 'the Group Output' : 'the output';
    for (const node of byId.values()) {
      if (reached.has(node.id) || node.type === GROUP_INPUT) continue;
      report(
        'unreachable-node',
        `${nodeName(node)} doesn't lead to ${target}, it never runs`,
        { nodeId: node.id }
      );
    }
  }

  private checkGroupCycles() {
    const onPath: number[] = [];
    const done = new Set<number>();
    const reported = new Set<number>();
    const visit = (group: GroupDefinition) => {
      onPath.push(group.id);
      for (const node of list(group.nodes)) {
        const nestedId = groupIdOf(node);
        const nested = nestedId === undefined ? undefined : this.groups.get(nestedId);
        if (!nested || done.has(nested.id)) continue;
        if (onPath.includes(nested.id)) {
          if (reported.has(nested.id)) continue;
          reported.add(nested.id);
          const circle = [...onPath.slice(onPath.indexOf(nested.id)), nested.id]
            .map(id => groupName(this.groups.get(id)!));
          this.issue('cycle', `a group contains itself: ${circle.join(' → ')}`, {
            nodeId: node.id,
            groupId: group.id
          });
          continue;
        }
        visit(nested);
      }
      onPath.pop();
      done.add(group.id);
    };
    for (const group of this.groups.values()) if (!done.has(group.id)) visit(group);
  }

  private checkOutputs(root: Scope) {
    const outputs = root.nodes.filter(node => isRootOutput(node.type));
    if (outputs.length === 0) {
      this.issue('no-output', `the graph has no output node, add ${OUTPUT}`, {
        suggestion: OUTPUT
      });
    } else if (outputs.length > 1) {
      this.issue(
        'several-outputs',
        `the graph has ${outputs.length} output nodes (${outputs.map(nodeName).join(', ')}), only ${
          nodeName(outputs[0])
        } is used`,
        { nodeId: outputs[1].id }
      );
    }
  }

  private checkSettings() {
    const settings = this.graph.settings;
    if (!settings || typeof settings !== 'object') return;
    const known = new Set<string>();
    for (const definition of this.definitions.values()) {
      for (const input of Object.values(definition.inputs ?? {})) {
        if (input.setting) known.add(input.setting);
      }
    }
    for (const key of Object.keys(settings)) {
      if (this.readSettings.has(key)) continue;
      const close = nearest(key, known.has(key) ? [] : [...known, 'seed']);
      this.issue(
        'unknown-setting',
        `no node of the graph reads the setting "${key}"${didYouMean(close)}`,
        { suggestion: close[0] }
      );
    }
  }
}

function isRootOutput(type: string) {
  return typeof type === 'string' && isOutputNode(type) && !type.startsWith(INTERNAL_PREFIX);
}

/**
 * Every mistake in a graph and in its groups, without running it: node types and inputs that
 * don't exist, edges between sockets that don't fit, values a node doesn't take and more, see
 * {@link GraphIssueCode}. `definitions` are the nodes the graph may use, by their id, the
 * internal nodes (groups and the debug node) are known without them.
 *
 * A graph of an older version is checked as it is after its migration. A graph without a
 * version is checked as it is written, it gets a `no-version` warning.
 */
export function validateGraph(
  graph: Graph,
  definitions: Map<string, NodeDefinition>
): GraphIssue[] {
  const issues: GraphIssue[] = [];
  if (graph.version === undefined) {
    issues.push({
      severity: 'warning',
      code: 'no-version',
      message:
        `the graph has no "version", add "version": ${CURRENT_GRAPH_VERSION}, without it the graph is migrated from version 0`,
      suggestion: `"version": ${CURRENT_GRAPH_VERSION}`
    });
  } else if (graph.version < CURRENT_GRAPH_VERSION) {
    graph = migrateGraph(graph);
  }
  return [...issues, ...new GraphValidator(graph, definitions).run()];
}
