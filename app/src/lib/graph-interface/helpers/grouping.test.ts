import { GROUP_INPUT, GROUP_INSTANCE, GROUP_OUTPUT } from '@nodarium/types';
import type { GroupDefinition, NodeInput, SerializedEdge, SerializedNode } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import { groupSelection, ungroupInstance } from './grouping';

type Level = { nodes: SerializedNode[]; edges: SerializedEdge[] };

const node = (
  id: number,
  position: [number, number] = [0, 0],
  extra: Partial<SerializedNode> = {}
): SerializedNode => ({ id, type: 'test/node/float', position, ...extra });

function counter(start: number) {
  return () => start++;
}

// every node: the inputs `a` and `b`, a float and a path output
const inputs: Record<string, NodeInput> = {
  a: { type: 'float', label: 'A', value: 0.1 },
  b: { type: 'float', value: 0.2 },
  path: { type: 'path', label: 'Path' }
};
const outputs = ['float', 'path'];

function group(level: Level, ids: number[], firstId = 100) {
  const grouped = groupSelection(level, new Set(ids), {
    outputType: (_, output) => outputs[output],
    input: (_, key) => inputs[key],
    nextId: counter(firstId)
  });
  if (!grouped) throw new Error('nothing was grouped');
  return grouped;
}

describe('groupSelection', () => {
  // 1 → 2 → 3
  const chain = (): Level => ({
    nodes: [node(1, [0, 0]), node(2, [100, 20]), node(3, [200, 0])],
    edges: [[1, 0, 2, 'a'], [2, 0, 3, 'a']]
  });

  it('groups nothing for an empty selection, unknown ids and the boundary nodes of a level', () => {
    const level: Level = {
      nodes: [
        node(1, [0, 0], { type: GROUP_INPUT }),
        node(2),
        node(3, [0, 0], { type: GROUP_OUTPUT })
      ],
      edges: [[1, 0, 2, 'a'], [2, 0, 3, 'out_0']]
    };
    const context = { outputType: () => 'float', input: () => undefined, nextId: counter(100) };
    expect(groupSelection(level, new Set(), context)).toBeUndefined();
    expect(groupSelection(level, new Set([99]), context)).toBeUndefined();
    expect(groupSelection(level, new Set([1, 3]), context)).toBeUndefined();
  });

  it('takes its ids in the order Group Input, Group Output, group, group node', () => {
    const { group: definition, instance } = group(chain(), [2]);
    expect(definition.nodes.map((n) => [n.id, n.type])).toEqual([
      [100, GROUP_INPUT],
      [2, 'test/node/float'],
      [101, GROUP_OUTPUT]
    ]);
    expect(definition.id).toBe(102);
    expect(instance).toEqual({
      id: 103,
      type: GROUP_INSTANCE,
      position: [100, 20],
      props: { groupId: 102 }
    });
  });

  it('makes an input of the socket that feeds the selection and an output of the one it feeds', () => {
    const { group: definition, instance, edges } = group(chain(), [2]);
    expect(definition.inputs).toEqual({ input_0: { type: 'float', label: 'A', value: 0.1 } });
    expect(definition.outputs).toEqual([{ type: 'float', label: 'Float' }]);
    expect(definition.edges).toEqual([[100, 0, 2, 'a'], [2, 0, 101, 'out_0']]);
    expect(edges).toEqual([[1, 0, instance.id, 'input_0'], [instance.id, 0, 3, 'a']]);
  });

  it('keeps an edge between two grouped nodes inside the group, not as an edge into the group node', () => {
    // 1 → 2 → 3 → 4, 2 and 3 are grouped
    const level: Level = {
      nodes: [node(1), node(2), node(3), node(4)],
      edges: [[1, 0, 2, 'a'], [2, 0, 3, 'a'], [3, 0, 4, 'a']]
    };
    const { group: definition, instance, edges } = group(level, [2, 3]);
    expect(definition.edges).toEqual([[100, 0, 2, 'a'], [2, 0, 3, 'a'], [3, 0, 101, 'out_0']]);
    expect(edges).toEqual([[1, 0, instance.id, 'input_0'], [instance.id, 0, 4, 'a']]);
    expect(Object.keys(definition.inputs!)).toEqual(['input_0']);
    expect(definition.outputs).toHaveLength(1);
  });

  it('makes one input of a socket that feeds several grouped nodes', () => {
    //     ┌→ 2
    // 1 ──┤       5 → 3.b
    //     └→ 3
    const level: Level = {
      nodes: [node(1), node(2), node(3), node(5)],
      edges: [[1, 0, 2, 'a'], [5, 1, 3, 'path'], [1, 0, 3, 'b']]
    };
    const { group: definition, instance, edges } = group(level, [2, 3]);
    expect(Object.keys(definition.inputs!)).toEqual(['input_0', 'input_1']);
    expect(definition.edges).toEqual([[100, 0, 2, 'a'], [100, 1, 3, 'path'], [100, 0, 3, 'b']]);
    expect(edges).toEqual([[1, 0, instance.id, 'input_0'], [5, 1, instance.id, 'input_1']]);
    // the last edge of the socket names the input
    expect(definition.inputs!.input_0).toEqual({ type: 'float', label: 'b', value: 0.2 });
    expect(definition.inputs!.input_1).toEqual({ type: 'path', label: 'Path' });
  });

  it('makes one output per internal source socket', () => {
    // 2 ┬→ 3      2 and 5 are grouped, output 0 of 2 fans out, its output 1 feeds 7
    //   ├→ 4
    //   └(path)→ 7
    // 5 ──→ 6
    const level: Level = {
      nodes: [node(2), node(5), node(3), node(4), node(6), node(7)],
      edges: [[2, 0, 3, 'a'], [2, 0, 4, 'b'], [5, 0, 6, 'a'], [2, 1, 7, 'path']]
    };
    const { group: definition, instance, edges } = group(level, [2, 5]);
    expect(definition.outputs).toEqual([
      { type: 'float', label: 'Float' },
      { type: 'float', label: 'Float' },
      { type: 'path', label: 'Path' }
    ]);
    expect(definition.edges).toEqual([
      [2, 0, 101, 'out_0'],
      [5, 0, 101, 'out_1'],
      [2, 1, 101, 'out_2']
    ]);
    expect(edges).toEqual([
      [instance.id, 0, 3, 'a'],
      [instance.id, 0, 4, 'b'],
      [instance.id, 1, 6, 'a'],
      [instance.id, 2, 7, 'path']
    ]);
  });

  it('leaves the edges alone that do not touch the selection', () => {
    const level = chain();
    level.nodes.push(node(8), node(9));
    level.edges.unshift([8, 0, 9, 'a']);
    const { edges } = group(level, [2]);
    expect(edges[0]).toEqual([8, 0, 9, 'a']);
    expect(edges).toHaveLength(3);
  });

  describe('the value of an input', () => {
    const grouped = (props: SerializedNode['props'], key = 'a', output = 0) => {
      const level: Level = {
        nodes: [node(1), node(2, [0, 0], props && { props })],
        edges: [[1, output, 2, key]]
      };
      return group(level, [2]).group.inputs!.input_0;
    };

    it('starts at the value of the internal node', () => {
      expect(grouped({ a: 0.25 }).value).toBe(0.25);
      expect(grouped({ a: 0 }).value).toBe(0);
    });

    it('starts at the default of the internal input when the node has no value', () => {
      expect(grouped(undefined).value).toBe(0.1);
      expect(grouped({ b: 3 }).value).toBe(0.1);
    });

    it('has no value when the socket has another type than the input it feeds', () => {
      // the path output into the float input `a`
      expect(grouped({ a: 0.25 }, 'a', 1)).toEqual({ type: 'path', label: 'A' });
    });

    it('is a copy, the group does not share it with the node', () => {
      const level: Level = {
        nodes: [node(1), node(2, [0, 0], { props: { a: [1, 2, 3] } })],
        edges: [[1, 0, 2, 'a']]
      };
      const { group: definition } = group(level, [2]);
      expect(definition.inputs!.input_0.value).toEqual([1, 2, 3]);
      expect(definition.inputs!.input_0.value).not.toBe(level.nodes[1].props!.a);
      expect(definition.nodes[1].props!.a).not.toBe(level.nodes[1].props!.a);
    });
  });

  it('names an input after its key and an output `Output` when nothing is known about them', () => {
    const grouped = groupSelection(chain(), new Set([2]), {
      outputType: () => undefined,
      input: () => undefined,
      nextId: counter(100)
    });
    expect(grouped?.group.inputs).toEqual({ input_0: { type: '*', label: 'a' } });
    expect(grouped?.group.outputs).toEqual([{ type: '*', label: 'Output' }]);
  });

  it('puts the group node at the centre of the selection and the boundary nodes at its sides', () => {
    const level: Level = {
      nodes: [node(1, [0, 0]), node(2, [100, 40]), node(3, [400, 10])],
      edges: []
    };
    const { group: definition, instance } = group(level, [1, 2, 3]);
    expect(instance.position).toEqual([500 / 3, 50 / 3]);
    expect(definition.nodes.at(0)?.position).toEqual([-50, 20]);
    expect(definition.nodes.at(-1)?.position).toEqual([425, 20]);
    expect(definition.nodes.slice(1, -1).map((n) => n.position)).toEqual([
      [0, 0],
      [100, 40],
      [400, 10]
    ]);
  });

  it('groups the nodes in the order of the selection, with what they carry', () => {
    const level: Level = {
      nodes: [
        node(1, [0, 0], { props: { a: 1 } }),
        node(2, [10, 0], { muted: true }),
        node(3, [20, 0], { exposed: ['b'] })
      ],
      edges: []
    };
    const { group: definition } = group(level, [3, 1, 2, 3]);
    expect(definition.nodes.slice(1, -1)).toEqual([level.nodes[2], level.nodes[0], level.nodes[1]]);
  });

  it('groups the nodes between the boundary nodes of an open group, which stay', () => {
    // inside a group: Group Input (1) → 2 → Group Output (3)
    const level: Level = {
      nodes: [
        node(1, [0, 0], { type: GROUP_INPUT }),
        node(2),
        node(3, [0, 0], { type: GROUP_OUTPUT })
      ],
      edges: [[1, 0, 2, 'a'], [2, 0, 3, 'out_0']]
    };
    const { group: definition, instance, edges } = group(level, [1, 2, 3]);
    expect(definition.nodes.map((n) => n.id)).toEqual([100, 2, 101]);
    expect(edges).toEqual([[1, 0, instance.id, 'input_0'], [instance.id, 0, 3, 'out_0']]);
  });

  it('leaves the level as it was', () => {
    const level = chain();
    const before = structuredClone(level);
    group(level, [2]);
    expect(level).toEqual(before);
  });
});

describe('ungroupInstance', () => {
  // Group Input (10) → 11 → 12 → Group Output (13)
  const definition = (): GroupDefinition => ({
    id: 20,
    inputs: {
      input_0: { type: 'float', label: 'A', value: 0.5 },
      // input_1 was removed, socket 1 of the Group Input node is input_2
      input_2: { type: 'float', label: 'B' }
    },
    outputs: [{ type: 'float' }, { type: 'path' }],
    nodes: [
      node(10, [-50, 0], { type: GROUP_INPUT }),
      node(11, [0, 0], { props: { a: 1, b: 2 } }),
      node(12, [100, 40], { exposed: ['b'] }),
      node(13, [125, 0], { type: GROUP_OUTPUT })
    ],
    edges: [
      [10, 0, 11, 'a'],
      [10, 1, 11, 'b'],
      [11, 0, 12, 'a'],
      [12, 0, 13, 'out_0'],
      [11, 1, 13, 'out_1']
    ]
  });

  const instance = (props: Record<string, number> = {}): SerializedNode =>
    node(30, [500, 300], { type: GROUP_INSTANCE, props: { groupId: 20, ...props } });

  // 1 → the instance (30) → 3 and 4
  const level = (edges: SerializedEdge[]): Level => ({
    nodes: [node(1), instance(), node(3), node(4)],
    edges
  });

  it('gives the nodes of the group new ids and leaves the boundary nodes behind', () => {
    const ids: number[] = [];
    const nextId = counter(200);
    const { nodes } = ungroupInstance(level([]), instance(), definition(), () => {
      ids.push(nextId());
      return ids.at(-1)!;
    });
    expect(nodes.map((n) => n.type)).toEqual(['test/node/float', 'test/node/float']);
    expect(new Set(nodes.map((n) => n.id)).size).toBe(2);
    for (const n of nodes) expect(ids).toContain(n.id);
  });

  it('keeps the layout of the nodes, their centre is where the group node was', () => {
    const { nodes } = ungroupInstance(level([]), instance(), definition(), counter(200));
    expect(nodes.map((n) => n.position)).toEqual([[450, 280], [550, 320]]);
  });

  it('replaces the edges of the group node', () => {
    const { nodes, edges } = ungroupInstance(
      level([[1, 0, 30, 'input_0'], [30, 0, 3, 'a'], [30, 1, 4, 'path'], [30, 0, 4, 'b']]),
      instance(),
      definition(),
      counter(200)
    );
    const [first, second] = nodes.map((n) => n.id);
    expect(edges).toEqual([
      [1, 0, first, 'a'],
      [first, 0, second, 'a'],
      [second, 0, 3, 'a'],
      [second, 0, 4, 'b'],
      [first, 1, 4, 'path']
    ]);
  });

  it('finds the input of a socket of the Group Input node by its position', () => {
    const { nodes, edges } = ungroupInstance(
      level([[1, 1, 30, 'input_2']]),
      instance(),
      definition(),
      counter(200)
    );
    expect(edges).toContainEqual([1, 1, nodes[0].id, 'b']);
    expect(edges).not.toContainEqual([1, 1, nodes[0].id, 'a']);
  });

  it('reads the output of the older `Out` socket as the first one', () => {
    const group = definition();
    group.edges = [[12, 0, 13, 'Out']];
    const { nodes, edges } = ungroupInstance(
      level([[30, 0, 3, 'a'], [30, 1, 4, 'path']]),
      instance(),
      group,
      counter(200)
    );
    expect(edges).toEqual([[nodes[1].id, 0, 3, 'a']]);
  });

  it('connects an input that the group passes straight to an output', () => {
    const group = definition();
    group.edges = [[10, 0, 13, 'out_0']];
    const { edges } = ungroupInstance(
      level([[1, 0, 30, 'input_0'], [30, 0, 3, 'a'], [30, 0, 4, 'b']]),
      instance(),
      group,
      counter(200)
    );
    expect(edges).toEqual([[1, 0, 3, 'a'], [1, 0, 4, 'b']]);
  });

  describe('an input of the group node that nothing is connected to', () => {
    const ungrouped = (props?: Record<string, number>) =>
      ungroupInstance(level([]), instance(props), definition(), counter(200));

    it('leaves the value of the group node on the node it fed', () => {
      const { nodes, edges } = ungrouped({ input_0: 0.75, input_2: 0 });
      expect(nodes[0].props).toEqual({ a: 0.75, b: 0 });
      expect(edges).toEqual([[nodes[0].id, 0, nodes[1].id, 'a']]);
    });

    it('leaves the default of the group input when the group node has no value', () => {
      expect(ungrouped().nodes[0].props).toEqual({ a: 0.5, b: 2 });
    });

    it('keeps the value of the node when neither has one', () => {
      expect(ungrouped({ input_0: 0.75 }).nodes[0].props?.b).toBe(2);
    });

    it('does not write the value of a connected input', () => {
      const { nodes } = ungroupInstance(
        level([[1, 0, 30, 'input_0']]),
        instance({ input_0: 0.75 }),
        definition(),
        counter(200)
      );
      expect(nodes[0].props).toEqual({ a: 1, b: 2 });
    });
  });

  it('copies what the nodes carry, the group keeps its own', () => {
    const group = definition();
    const { nodes } = ungroupInstance(level([]), instance(), group, counter(200));
    expect(nodes[1].exposed).toEqual(['b']);
    expect(nodes[1].props).toEqual({});
    nodes[0].props!.a = 99;
    nodes[1].exposed!.push('a');
    expect(group).toEqual(definition());
  });

  it('keeps a muted node muted', () => {
    const group = definition();
    group.nodes[2].muted = true;
    const { nodes } = ungroupInstance(level([]), instance(), group, counter(200));
    expect(nodes.map((n) => n.muted)).toEqual([undefined, true]);
  });

  it('brings out a group without nodes as nothing', () => {
    const group = definition();
    group.nodes = [group.nodes[0], group.nodes[3]];
    group.edges = [];
    expect(ungroupInstance(level([]), instance(), group, counter(200))).toEqual({
      nodes: [],
      edges: []
    });
  });
});
