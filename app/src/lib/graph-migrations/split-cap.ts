import type { Graph, SerializedEdge, SerializedNode } from '@nodarium/types';
import type { InputSocket, OutputSocket, ReplaceContext, Replacement } from './helpers';

const CAP = 'max/myzel/cap';

/** The inputs of max/myzel/cap before graph version 3, with their defaults then */
const OLD_DEFAULTS = {
  thickness: 0.008,
  thickness_curve: [0, 1, 0.6, 0.6, 1, 0.15],
  margin_curl: 0.2,
  opening: 1,
  waves: 6,
  wave_amplitude: 0,
  wave_irregularity: 0.3,
  wave_profile: [0, 0, 0.5, 0.25, 1, 1],
  spacing: 0.5
};

/** Inputs the cap still has */
const KEPT = ['plant', 'radius', 'profile', 'rays', 'seed', 'resolution_curve'];

/** How far apart the new nodes are placed, to the right of the cap */
const STEP = 25;

type Props = NonNullable<SerializedNode['props']>;

function sameValue(a: unknown, b: unknown) {
  return JSON.stringify(a) === JSON.stringify(b);
}

/** The inputs of every cap that have an edge into them, by node */
export function wiredCapInputs(graph: Graph) {
  const wired = new Map<SerializedNode, Set<string>>();
  for (const scope of [graph, ...(graph.groups ?? [])]) {
    const caps = new Map(scope.nodes.filter((n) => n.type === CAP).map((n) => [n.id, n]));
    for (const edge of scope.edges) {
      const cap = caps.get(edge[2]);
      if (!cap) continue;
      if (!wired.has(cap)) wired.set(cap, new Set());
      wired.get(cap)!.add(edge[3]);
    }
  }
  return wired;
}

/**
 * The cap of graph version 2 posed its rays itself. From version 3 on it only grows the open
 * cap, and fill_gaps, margin_waves, margin_curl and cap_open pose it, in that order. An old cap
 * becomes the new one, a curve for its thickness from the centre to the margin, and the pose
 * nodes whose old inputs did something: values that differ from the old defaults, or inputs
 * that had an edge into them. Its rays and margin come out of the last of them, its hinge out
 * of the cap. The templates were converted the same way.
 */
export function splitCap(
  old: SerializedNode,
  { nextId }: ReplaceContext,
  wired: Set<string> = new Set()
): Replacement {
  const props: Props = old.props ?? {};
  const value = <K extends keyof typeof OLD_DEFAULTS>(key: K) =>
    (props[key] ?? OLD_DEFAULTS[key]) as (typeof OLD_DEFAULTS)[K];
  const used = (key: keyof typeof OLD_DEFAULTS, idle: unknown = OLD_DEFAULTS[key]) =>
    wired.has(key) || !sameValue(value(key), idle);

  const [x, y] = old.position;
  let step = 0;
  const place = (): [number, number] => [x + STEP * ++step, y];

  const cap: SerializedNode = {
    ...old,
    props: Object.fromEntries(Object.entries(props).filter(([key]) => KEPT.includes(key)))
  };
  const nodes: SerializedNode[] = [cap];
  const edges: SerializedEdge[] = [];
  const inputs: Record<string, InputSocket[]> = Object.fromEntries(
    KEPT.map((key) => [key, [[old.id, key] as InputSocket]])
  );

  // the thickness, thinned towards the margin by the old thickness curve
  const thickness = nextId();
  nodes.push({
    id: thickness,
    type: 'max/plantarium/curve',
    position: [x - STEP, y + STEP],
    props: { curve: value('thickness_curve'), min: 0, max: value('thickness') }
  });
  edges.push([thickness, 0, old.id, 'thickness']);
  inputs.thickness = [[thickness, 'max']];

  let rays: OutputSocket = [old.id, 0];
  let margin: OutputSocket = [old.id, 2];
  let posed = false;
  const pose = (type: string, nodeProps: Props, input: string, hinge = true) => {
    const id = nextId();
    nodes.push({ id, type: type as SerializedNode['type'], position: place(), props: nodeProps });
    edges.push([rays[0], rays[1], id, input]);
    if (hinge) edges.push([old.id, 1, id, 'hinge']);
    rays = [id, 0];
    if (hinge) {
      margin = [id, 1];
      posed = true;
    }
    return id;
  };

  if (used('spacing', 0)) {
    const id = pose('max/plantarium/fill_gaps', { spacing: value('spacing') }, 'plant', false);
    inputs.spacing = [[id, 'spacing']];
  }

  if (used('wave_amplitude', 0)) {
    const waves = pose(
      'max/myzel/margin_waves',
      { waves: value('waves'), irregularity: value('wave_irregularity') },
      'cap'
    );
    inputs.waves = [[waves, 'waves']];
    inputs.wave_irregularity = [[waves, 'irregularity']];
    if (used('wave_profile')) {
      // the amplitude follows the old profile from the axis (alpha 0) to the margin
      const amplitude = nextId();
      nodes.push({
        id: amplitude,
        type: 'max/plantarium/curve',
        position: [x + STEP * step, y + STEP],
        props: { curve: value('wave_profile'), min: 0, max: value('wave_amplitude') }
      });
      edges.push([amplitude, 0, waves, 'amplitude']);
      inputs.wave_amplitude = [[amplitude, 'max']];
    } else {
      // a plain amplitude spreads like the old default profile
      nodes.at(-1)!.props!.amplitude = value('wave_amplitude');
      inputs.wave_amplitude = [[waves, 'amplitude']];
    }
  }

  if (used('margin_curl', 0)) {
    const id = pose('max/myzel/margin_curl', { amount: value('margin_curl') }, 'cap');
    inputs.margin_curl = [[id, 'amount']];
  }

  if (used('opening')) {
    const id = pose('max/myzel/cap_open', { opening: value('opening') }, 'cap');
    inputs.opening = [[id, 'opening']];
  } else if (!posed && rays[0] !== old.id) {
    // the margin of every ray fill_gaps added, an open cap_open changes nothing else
    pose('max/myzel/cap_open', {}, 'cap');
  }

  return { nodes, edges, inputs, outputs: { 0: rays, 1: [old.id, 1], 2: margin } };
}
