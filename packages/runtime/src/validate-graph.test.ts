import {
  DEBUG_NODE,
  type Graph,
  GROUP_INPUT,
  GROUP_INSTANCE,
  GROUP_OUTPUT,
  type GroupDefinition,
  type NodeDefinition,
  type NodeValue
} from '@nodarium/types';
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { createRuntime } from './create-runtime';
import { GraphValidationError } from './errors';
import { CURRENT_GRAPH_VERSION } from './graph-migrations';
import { createMockNodeRegistry } from './test-utils';
import { type GraphIssue, validateGraph } from './validate-graph';

function nodeDefinition(name: string, file = 'definition.json'): NodeDefinition {
  const url = new URL(`../../../nodes/max/plantarium/${name}/src/${file}`, import.meta.url);
  return JSON.parse(readFileSync(url, 'utf8'));
}

const definitions = new Map(
  [
    nodeDefinition('output', 'inputs.json'),
    nodeDefinition('stem'),
    nodeDefinition('noise'),
    nodeDefinition('float'),
    nodeDefinition('random')
  ].map(d => [d.id as string, d])
);

/** stem → noise → output, as written by hand */
function threeNodes(): Graph {
  return {
    id: 0,
    version: CURRENT_GRAPH_VERSION,
    settings: { 'resolution.circle': 12, 'resolution.curve': 16 },
    nodes: [
      { id: 0, type: 'max/plantarium/output', position: [200, 0], props: {} },
      {
        id: 1,
        type: 'max/plantarium/stem',
        position: [0, 0],
        props: { length: 2, thickness: 0.1 }
      },
      { id: 2, type: 'max/plantarium/noise', position: [100, 0], props: { strength: 0.5 } }
    ],
    edges: [[1, 0, 2, 'plant'], [2, 0, 0, 'input']],
    groups: []
  } as Graph;
}

function withMistake(change: (graph: Graph) => void) {
  const graph = threeNodes();
  change(graph);
  return graph;
}

function codes(issues: GraphIssue[]) {
  return issues.map(issue => `${issue.severity} ${issue.code}`);
}

const mistakes: Record<string, (graph: Graph) => void> = {
  'no version': (g) => delete g.version,
  'node type max/plantarium/nois': (g) => (g.nodes[2].type = 'max/plantarium/nois'),
  'edge into the input inputt of the output': (g) => (g.edges[1][3] = 'inputt'),
  'prop thicknes on the stem': (g) => (g.nodes[1].props = { length: 2, thicknes: 0.1 }),
  'strength 99999 on noise': (g) => (g.nodes[2].props = { strength: 99999 }),
  'a float into the plant input of noise': (g) => {
    g.nodes.push({ id: 3, type: 'max/plantarium/float', position: [0, 100], props: {} });
    g.edges[0] = [3, 0, 2, 'plant'];
  },
  'a path into the strength input of noise': (g) => g.edges.push([1, 0, 2, 'strength']),
  'edge from node 99': (g) => (g.edges[0][0] = 99),
  'edge from output 5 of the stem': (g) => (g.edges[0][1] = 5),
  'no output node': (g) => {
    g.nodes.shift();
    g.edges.pop();
  }
};

describe('validateGraph', () => {
  it('finds nothing in a graph without mistakes', () => {
    expect(validateGraph(threeNodes(), definitions)).toEqual([]);
  });

  // the table of #400, one mistake at a time
  it.each([
    ['no version', 'warning no-version'],
    ['node type max/plantarium/nois', 'error unknown-node-type'],
    ['edge into the input inputt of the output', 'error unknown-input'],
    ['prop thicknes on the stem', 'error unknown-prop'],
    ['strength 99999 on noise', 'warning value-out-of-range'],
    ['a float into the plant input of noise', 'error incompatible-edge'],
    ['a path into the strength input of noise', 'error incompatible-edge'],
    ['edge from node 99', 'error missing-node'],
    ['edge from output 5 of the stem', 'error output-out-of-range'],
    ['no output node', 'error no-output']
  ])('names %s', (mistake, code) => {
    expect(codes(validateGraph(withMistake(mistakes[mistake]), definitions))).toContain(code);
  });

  it('says what is wrong and how to fix it', () => {
    const messages = (mistake: string) =>
      validateGraph(withMistake(mistakes[mistake]), definitions).map(i => i.message);
    expect(messages('no version')).toEqual([
      `the graph has no "version", add "version": ${CURRENT_GRAPH_VERSION}, without it the graph is migrated from version 0`
    ]);
    expect(messages('node type max/plantarium/nois')).toEqual([
      'nois#2 has the unknown type "max/plantarium/nois", did you mean "max/plantarium/noise"?'
    ]);
    expect(messages('edge into the input inputt of the output')).toEqual([
      'output#0 has no input "inputt", did you mean "input"?'
    ]);
    expect(messages('prop thicknes on the stem')).toEqual([
      'stem#1 has no input "thicknes", did you mean "thickness"?'
    ]);
    expect(messages('strength 99999 on noise')).toEqual([
      'noise#2 "strength" is 99999, far above the range 0 to 1 of the input'
    ]);
    expect(messages('a float into the plant input of noise')).toContain(
      'noise#2 "plant" takes path, output 0 of float#3 is float'
    );
    expect(messages('edge from node 99')).toContain(
      'the edge into noise#2 "plant" comes from node 99, there is no node 99'
    );
    expect(messages('edge from output 5 of the stem')).toEqual([
      'the edge into noise#2 "plant" reads output 5 of stem#1, which only has output 0'
    ]);
  });

  it('points at the node, the edge and the fix', () => {
    const [unknown] = validateGraph(
      withMistake(mistakes['prop thicknes on the stem']),
      definitions
    );
    expect(unknown).toMatchObject({ nodeId: 1, suggestion: 'thickness' });
    const [edge] = validateGraph(
      withMistake(mistakes['edge into the input inputt of the output']),
      definitions
    );
    expect(edge).toMatchObject({ nodeId: 0, edge: [2, 0, 0, 'inputt'], suggestion: 'input' });
    const [type] = validateGraph(
      withMistake(mistakes['node type max/plantarium/nois']),
      definitions
    );
    expect(type.suggestion).toBe('max/plantarium/noise');
  });

  it('finds a node of the same name in another collection', () => {
    const graph = withMistake((g) => (g.nodes[2].type = 'max/myzel/noise'));
    expect(validateGraph(graph, definitions)[0].suggestion).toBe('max/plantarium/noise');
  });

  it('names all mistakes of a graph at once', () => {
    const graph = withMistake((g) => {
      for (
        const mistake of [
          'no version',
          'edge into the input inputt of the output',
          'prop thicknes on the stem',
          'strength 99999 on noise',
          'edge from output 5 of the stem'
        ]
      ) {
        mistakes[mistake](g);
      }
      // into the scale, the edge would hide the value of the strength
      g.edges.push([1, 0, 2, 'scale']);
    });
    expect(codes(validateGraph(graph, definitions)).sort()).toEqual([
      'error incompatible-edge',
      'error output-out-of-range',
      'error unknown-input',
      'error unknown-prop',
      'warning no-version',
      'warning value-out-of-range'
    ]);
  });

  it('checks a graph without a version as it is written', () => {
    // migration 2 would rename the old fixBottom, the input of today is fix_bottom
    const graph = withMistake((g) => {
      delete g.version;
      g.nodes[2].props = { fix_bottom: 0.5 };
    });
    expect(codes(validateGraph(graph, definitions))).toEqual(['warning no-version']);
  });

  it('checks a graph of an older version as it is after its migration', () => {
    const graph = withMistake((g) => {
      g.version = 1;
      g.nodes[2].props = { fixBottom: 0.5 };
    });
    expect(validateGraph(graph, definitions)).toEqual([]);
  });

  it('names nodes that share an id, inputs connected twice and several outputs', () => {
    const graph = withMistake((g) => {
      g.nodes.push({ id: 2, type: 'max/plantarium/stem', position: [0, 0] });
      g.nodes.push({ id: 5, type: 'max/plantarium/output', position: [0, 0] });
      g.edges.push([1, 0, 0, 'input']);
    });
    expect(codes(validateGraph(graph, definitions))).toEqual([
      'error duplicate-id',
      'error input-connected-twice',
      'error several-outputs'
    ]);
  });

  it('names a cycle', () => {
    const graph = withMistake((g) => {
      g.nodes.push({ id: 3, type: 'max/plantarium/noise', position: [0, 0] });
      g.edges = [[1, 0, 2, 'plant'], [2, 0, 3, 'plant'], [3, 0, 2, 'plant'], [3, 0, 0, 'input']];
    });
    const issues = validateGraph(graph, definitions);
    expect(issues.find(i => i.code === 'cycle')?.message).toBe(
      'the nodes depend on each other in a circle: noise#2 → noise#3 → noise#2'
    );
    expect(codes(issues)).toContain('error input-connected-twice');
  });

  it('names values of the wrong type, props that are never read and unknown settings', () => {
    const graph = withMistake((g) => {
      g.nodes[2].props = { strength: [1, 2], seed: 4 };
      g.nodes[1].props = { resolution_curve: 8, origin: 1 };
      g.settings = { 'resolution.curv': 16 };
    });
    expect(validateGraph(graph, definitions).map(i => [i.code, i.message])).toEqual([
      [
        'unknown-prop',
        'stem#1 "resolution_curve" takes its value from the setting "resolution.curve", the prop is never read, remove it'
      ],
      ['wrong-value-type', 'stem#1 "origin" is 1, it takes three numbers'],
      ['wrong-value-type', 'noise#2 "strength" is [1,2], it takes a number'],
      [
        'unknown-prop',
        'noise#2 "seed" takes the seed of the graph, the prop is never read, remove it'
      ],
      [
        'unknown-setting',
        'no node of the graph reads the setting "resolution.curv", did you mean "resolution.curve"?'
      ]
    ]);
  });

  it('leaves the value of a connected input alone, the editor keeps it', () => {
    const graph = withMistake((g) => (g.nodes[2].props = { plant: 0 }));
    expect(validateGraph(graph, definitions)).toEqual([]);
  });

  it('warns about nodes that never run', () => {
    const graph = withMistake((g) => {
      g.nodes.push({ id: 3, type: 'max/plantarium/random', position: [0, 0] });
      g.nodes.push({ id: 4, type: 'max/plantarium/stem', position: [0, 0] });
      g.nodes.push({ id: 5, type: DEBUG_NODE, position: [0, 0] });
      g.edges.push([4, 0, 5, 'input']);
    });
    expect(validateGraph(graph, definitions)).toEqual([
      {
        severity: 'warning',
        code: 'unreachable-node',
        message: "random#3 doesn't lead to the output, it never runs",
        nodeId: 3
      }
    ]);
  });

  describe('groups', () => {
    const group: GroupDefinition = {
      id: 7,
      name: 'Wobble',
      nodes: [
        { id: 10, type: GROUP_INPUT, position: [0, 0] },
        { id: 11, type: GROUP_OUTPUT, position: [0, 0] },
        { id: 12, type: 'max/plantarium/noise', position: [0, 0] }
      ],
      edges: [[10, 0, 12, 'plant'], [10, 1, 12, 'strength'], [12, 0, 11, 'out_0']],
      inputs: {
        plant: { type: 'path' },
        strength: { type: 'float', value: 0.5, min: 0, max: 1 }
      },
      outputs: [{ type: 'path' }]
    };

    function grouped(change?: (graph: Graph, group: GroupDefinition) => void): Graph {
      const g = structuredClone(group);
      const graph = withMistake((graph) => {
        graph.nodes[2] = {
          id: 2,
          type: GROUP_INSTANCE,
          position: [100, 0],
          props: { groupId: 7, strength: 0.3 }
        };
        graph.groups = [g];
      });
      change?.(graph, g);
      return graph;
    }

    it('takes the sockets of an instance and of the boundary nodes from the group', () => {
      expect(validateGraph(grouped(), definitions)).toEqual([]);
    });

    it('names the mistakes inside a group with the group', () => {
      const graph = grouped((_, g) => {
        g.edges = [[10, 1, 12, 'plant'], [10, 2, 12, 'scale'], [12, 0, 11, 'out_1']];
      });
      expect(validateGraph(graph, definitions).map(i => [i.code, i.message, i.groupId])).toEqual([
        [
          'incompatible-edge',
          'noise#12 "plant" takes path, output 1 of group_input#10 is float (in group "Wobble")',
          7
        ],
        [
          'output-out-of-range',
          'the edge into noise#12 "scale" reads output 2 of group_input#10, which has the outputs 0 to 1 (in group "Wobble")',
          7
        ],
        [
          'unknown-input',
          'group_output#11 has no input "out_1", did you mean "out_0"? (in group "Wobble")',
          7
        ]
      ]);
    });

    it('checks the props of an instance against the inputs of its group', () => {
      const graph = grouped((graph) => (graph.nodes[2].props = { groupId: 7, strenght: 0.3 }));
      expect(validateGraph(graph, definitions).map(i => i.message)).toEqual([
        'group#2 has no input "strenght", did you mean "strength"?'
      ]);
    });

    it("names an instance of a group the graph doesn't have", () => {
      const graph = grouped((graph) => (graph.nodes[2].props = { groupId: 8 }));
      expect(validateGraph(graph, definitions).map(i => i.message)).toEqual([
        'group#2 uses the group 8, the graph has no such group (groups: 7)'
      ]);
    });

    it('names a group that contains itself', () => {
      const graph = grouped((_, g) => {
        g.nodes.push({ id: 13, type: GROUP_INSTANCE, position: [0, 0], props: { groupId: 7 } });
      });
      expect(validateGraph(graph, definitions)).toContainEqual(
        expect.objectContaining({
          code: 'cycle',
          message: 'a group contains itself: group "Wobble" → group "Wobble"',
          groupId: 7
        })
      );
    });
  });
});

describe('createRuntime({ validate: true })', () => {
  const output = {
    id: 'max/plantarium/output',
    inputs: { input: { type: 'path' } },
    outputs: [],
    execute: (inputs: NodeValue[]) => inputs[0]
  } as unknown as NodeDefinition;
  const stem = {
    id: 'max/plantarium/stem',
    inputs: { length: { type: 'float', value: 1 } },
    outputs: ['path'],
    execute: () => Int32Array.of(0, 1, 1, 1)
  } as unknown as NodeDefinition;
  const graph = {
    id: 0,
    version: CURRENT_GRAPH_VERSION,
    nodes: [
      { id: 0, type: output.id, position: [0, 0] },
      { id: 1, type: stem.id, position: [0, 0], props: { lenght: 2 } }
    ],
    edges: [[1, 0, 0, 'input']],
    groups: []
  } as Graph;

  it('throws a GraphValidationError with all issues before anything runs', async () => {
    const runtime = createRuntime({
      registry: createMockNodeRegistry([output, stem]),
      validate: true
    });
    const error = await runtime.execute(graph, {}).catch(e => e);
    expect(error).toBeInstanceOf(GraphValidationError);
    expect(error.message).toBe(
      'The graph is not valid: stem#1 has no input "lenght", did you mean "length"?'
    );
    expect(error.issues).toHaveLength(1);
  });

  it('runs a graph with mistakes as before without the option', async () => {
    const runtime = createRuntime({ registry: createMockNodeRegistry([output, stem]) });
    expect([...await runtime.execute(graph, {})]).toEqual([0, 1, 1, 1]);
  });

  it('runs a graph that only has warnings', async () => {
    const runtime = createRuntime({
      registry: createMockNodeRegistry([output, stem]),
      validate: true
    });
    const { version: _, ...unversioned } = {
      ...graph,
      nodes: [graph.nodes[0], { ...graph.nodes[1], props: {} }]
    };
    expect([...await runtime.execute(unversioned as Graph, {})]).toEqual([0, 1, 1, 1]);
  });
});
