import {
  type Graph,
  GROUP_INPUT,
  GROUP_INSTANCE,
  GROUP_OUTPUT,
  type GroupDefinition,
  groupIdOf
} from '@nodarium/types';

export const substrateKinds = ['stump', 'moss', 'needles', 'leaves'] as const;
export type SubstrateKind = (typeof substrateKinds)[number];

export type Substrate = {
  /** The scan of the `max/myzel/mesh` node */
  kind: SubstrateKind;
  /** How many places on the substrate the stipes start at, the stipes take them in turn */
  places: number;
  /** How far apart the places keep at least, in meters */
  spacing: number;
  /** How level a place has to be, the `facing_up` of `sample_mesh` */
  facingUp: number;
};

/**
 * The substrates the mushroom templates used to grow on, for `withSubstrate()`.
 * The templates themselves come without one.
 */
export const substrates: Partial<Record<string, Substrate>> = {
  porcini: { kind: 'moss', places: 1, spacing: 0, facingUp: 0.95 },
  chanterelle: { kind: 'moss', places: 6, spacing: 0.05, facingUp: 0.85 },
  'honey-fungus': { kind: 'stump', places: 2, spacing: 0.12, facingUp: 0.93 },
  'sulphur-tuft': { kind: 'stump', places: 2, spacing: 0.1, facingUp: 0.93 },
  'false-chanterelle': { kind: 'needles', places: 7, spacing: 0.06, facingUp: 0.85 },
  'bitter-bolete': { kind: 'needles', places: 1, spacing: 0, facingUp: 0.95 }
};

function substrateGroup(id: number, substrate: Substrate): GroupDefinition {
  const [input, output, mesh, sample] = [id + 1, id + 2, id + 3, id + 4];
  return {
    id,
    name: 'Substrate',
    nodes: [
      { id: input, position: [-90, 5], type: GROUP_INPUT, props: {} },
      { id: output, position: [0, 0], type: GROUP_OUTPUT, props: {} },
      { id: mesh, position: [-60, 5], type: 'max/myzel/mesh', props: { substrate: 0, colored: 1 } },
      {
        id: sample,
        position: [-30, 5],
        type: 'max/myzel/sample_mesh',
        props: { amount: 1, spacing: 0, facing_up: substrate.facingUp }
      }
    ],
    edges: [
      [input, 0, mesh, 'substrate'],
      [input, 1, mesh, 'colored'],
      [input, 2, sample, 'amount'],
      [input, 3, sample, 'spacing'],
      [mesh, 0, sample, 'mesh'],
      [mesh, 0, output, 'out_0'],
      [sample, 0, output, 'out_1']
    ],
    inputs: {
      substrate: {
        type: 'select',
        label: 'Substrate',
        options: [...substrateKinds],
        value: 0,
        description: 'What the mushrooms grow on'
      },
      colored: {
        type: 'boolean',
        label: 'Colored',
        value: true,
        description:
          'Off shows the substrate in the material of the viewer, so the mushrooms stand out'
      },
      places: {
        type: 'integer',
        label: 'Places',
        value: 1,
        min: 1,
        max: 50,
        description:
          'How many places on the substrate the stipes start at, the stipes take them in turn'
      },
      spacing: {
        type: 'float',
        label: 'Spacing',
        value: 0,
        min: 0,
        max: 0.3,
        step: 0.005,
        unit: 'm',
        description: 'How far apart the places keep at least'
      }
    },
    outputs: [{ type: 'mesh', label: 'Substrate' }, { type: 'points', label: 'Places' }]
  };
}

/**
 * A copy of a mushroom graph that grows on a substrate. A Substrate group joins its mesh into
 * the last input of the `join_meshes` before the `output`, so the substrate is the highest
 * material group of the mesh, and gives its places to the `points` of every group that takes
 * them, like the Stipe group of the mushroom templates. Its instance is the one node with a
 * `substrate` prop.
 */
export function withSubstrate(template: Graph, substrate: Substrate): Graph {
  const graph = structuredClone(template);
  const output = graph.nodes.find((n) => n.type === 'max/myzel/output');
  const joinId = graph.edges.find(([, , to, input]) => to === output?.id && input === 'mesh')?.[0];
  const join = graph.nodes.find((n) => n.id === joinId);
  if (join?.type !== 'max/myzel/join_meshes') {
    throw new Error('The graph has no join_meshes before its output');
  }
  if (graph.edges.some(([, , to, input]) => to === join.id && input === 'd')) {
    throw new Error('The last input of join_meshes is taken');
  }

  const takesPoints = new Set(
    graph.groups.filter((g) => g.inputs?.points?.type === 'points').map((g) => g.id)
  );
  const stipes = graph.nodes.filter((n) => {
    const groupId = groupIdOf(n);
    return groupId !== undefined && takesPoints.has(groupId)
      && !graph.edges.some(([, , to, input]) => to === n.id && input === 'points');
  });
  if (!stipes.length) throw new Error('No group of the graph takes points');

  const ids = [
    ...graph.nodes.map((n) => n.id),
    ...graph.groups.flatMap((g) => [g.id, ...g.nodes.map((n) => n.id)])
  ];
  const groupId = Math.max(0, ...ids) + 1;
  const instanceId = groupId + 5;
  graph.groups.push(substrateGroup(groupId, substrate));
  graph.nodes.push({
    id: instanceId,
    position: [join.position[0] - 60, join.position[1] + 40],
    type: GROUP_INSTANCE,
    props: {
      groupId,
      substrate: substrateKinds.indexOf(substrate.kind),
      colored: 1,
      places: substrate.places,
      spacing: substrate.spacing
    }
  });
  graph.edges.push([instanceId, 0, join.id, 'd']);
  for (const stipe of stipes) graph.edges.push([instanceId, 1, stipe.id, 'points']);
  return graph;
}
