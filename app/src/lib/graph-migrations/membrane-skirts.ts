import type { Graph, SerializedEdge, SerializedNode } from '@nodarium/types';
import {
  type InputSocket,
  type ReplaceContext,
  type Replacement,
  replaceNodes,
  wiredInputs
} from './helpers';

const MEMBRANE = 'max/myzel/membrane';
const SKIRT = 'max/myzel/skirt';
const JOIN = 'max/myzel/join_meshes';
const POINTS = 'max/plantarium/points_on_path';
const PATH_INFO = 'max/plantarium/path_info';
const CURVE = 'max/plantarium/curve';
const MATH = 'max/plantarium/math';

const DIVIDE = 3;
const PATH_LENGTH = 5;

/** What membrane had before graph version 13 when nothing was saved */
const OLD_DEFAULTS = {
  attachment: 0,
  anchor: 0,
  along: 0,
  thickness: 0.001,
  tear: 0,
  tear_position: 0.95,
  torn_length: 0,
  flare: [0, 0.45, 0.4, 0.25, 1, 0.12]
};

/** The tear at which half of the strands had torn: the membrane gives way to the skirts there */
const SWITCH = 0.25;
/** The tear at which the torn parts had about fallen */
const FALLEN = 0.8;
/** How far a part sticks out (flare) when it has just torn */
const JUST_TORN = 0.55;
/** A torn part shrank by this share of its length as it fell */
const RECOIL = 0.3;
/** Torn parts shorter than this share of the span were left out */
const MIN_PART = 0.03;
/** The span the torn lengths are taken from when the membrane stretched with it */
const SPAN = 0.01;
/** Where the ring goes on the path in b when the membrane followed the level of a */
const RING_ALONG = 0.75;

const MEMBRANE_INPUTS = ['a', 'b', 'thickness', 'sag', 'resolution_curve', 'creator'];

/** How far apart the new nodes are placed, below the membrane */
const STEP = 25;

/** A step from 1 to 0 at `SWITCH` of the tear, or from 0 to 1 */
const step = (down: boolean) =>
  down ? [0, 1, SWITCH, 1, SWITCH + 0.01, 0, 1, 0] : [0, 0, SWITCH, 0, SWITCH + 0.01, 1, 1, 1];

const round = (x: number) => Math.round(x * 1e6) / 1e6;

/**
 * Up to graph version 12 membrane placed itself on the path in b with attachment, anchor and
 * along, and tore into a ring and a fringe with tear, tear_position, torn_length and its own
 * flare curve. From version 13 on it is only the closed skin between a and b, and wraps around a
 * point in b. A skirt hangs a torn part from one attachment.
 *
 * - along becomes a points_on_path on b with lowest and highest point at along.
 * - anchor becomes the same with anchor ÷ the length of the path, from a math and a path_info.
 * - Without either, the membrane keeps its path in b and still wraps level with a. attachment
 *   can't be drawn and is dropped.
 * - A membrane that tore (tear from a node, or a plain tear past `SWITCH`) gets a skirt for the
 *   ring on the point in b and one for the fringe on a, their meshes joined with the membrane's.
 *   The ring is torn_length × tear_position long, less the recoil, the fringe gets the rest. A
 *   tear from a node turns the membrane off and the skirts on at `SWITCH`, by its thickness and
 *   their length, and lets the skirts fall from sticking out to their flare by `FALLEN`.
 * - The flare curve becomes a curve node into the flare of the skirts.
 */
function replace(
  old: SerializedNode,
  { nextId }: ReplaceContext,
  wired: Set<string> = new Set()
): Replacement {
  const props = old.props ?? {};
  const value = <K extends keyof typeof OLD_DEFAULTS>(key: K) =>
    (props[key] ?? OLD_DEFAULTS[key]) as (typeof OLD_DEFAULTS)[K];
  const [x, y] = old.position;
  const nodes: SerializedNode[] = [];
  const edges: SerializedEdge[] = [];
  const inputs: Record<string, InputSocket[]> = {};
  const add = (
    type: string,
    nodeProps: NonNullable<SerializedNode['props']>,
    exposed?: string[]
  ) => {
    const id = nextId();
    const node: SerializedNode = {
      id,
      position: [x, y + STEP * (nodes.length + 1)],
      type: type as SerializedNode['type'],
      props: nodeProps
    };
    if (exposed) node.exposed = exposed;
    nodes.push(node);
    return id;
  };
  const into = (key: string, ...sockets: InputSocket[]) => {
    inputs[key] = [...(inputs[key] ?? []), ...sockets];
  };

  const membrane: SerializedNode = {
    ...old,
    props: Object.fromEntries(
      Object.entries(props).filter(([key]) => MEMBRANE_INPUTS.includes(key))
    )
  };
  if (old.exposed) membrane.exposed = old.exposed.filter((key) => MEMBRANE_INPUTS.includes(key));
  for (const key of MEMBRANE_INPUTS) into(key, [old.id, key]);

  // the spot on b, as a point
  const placeAt = (alpha: number | undefined) => {
    const points = add(
      POINTS,
      alpha === undefined
        ? { amount: 1 }
        : { amount: 1, lowest_point: alpha, highest_point: alpha },
      alpha === undefined ? ['lowest_point', 'highest_point'] : undefined
    );
    into('b', [points, 'plant']);
    return points;
  };
  const spread = (from: number, points: number) =>
    edges.push([from, 0, points, 'lowest_point'], [from, 0, points, 'highest_point']);
  let spot: number | undefined;
  if (wired.has('along')) {
    spot = placeAt(undefined);
    into('along', [spot, 'lowest_point'], [spot, 'highest_point']);
  } else if (Number(value('along')) > 0) {
    spot = placeAt(Math.min(Number(value('along')), 1));
  } else if (wired.has('anchor') || Number(value('anchor')) > 0) {
    spot = placeAt(undefined);
    const length = add(PATH_INFO, { info: PATH_LENGTH });
    const share = add(
      MATH,
      wired.has('anchor') ? { op_type: DIVIDE } : { op_type: DIVIDE, a: Number(value('anchor')) }
    );
    edges.push([length, 0, share, 'b']);
    spread(share, spot);
    if (wired.has('anchor')) into('anchor', [share, 'a']);
  }
  if (spot !== undefined) {
    inputs.b = inputs.b.filter(([id]) => id !== old.id);
    edges.push([spot, 0, old.id, 'b']);
  }

  const tearWired = wired.has('tear');
  const tear = Number(value('tear'));
  if (!tearWired && tear < SWITCH) {
    return { nodes: [membrane, ...nodes], edges, inputs, outputs: { 0: [old.id, 0] } };
  }

  // the ring hangs from the spot on b, the fringe from a
  const ringSpot = spot ?? placeAt(RING_ALONG);
  const tornLength = Number(value('torn_length')) > 0 ? Number(value('torn_length')) : SPAN;
  const cut = wired.has('tear_position')
    ? OLD_DEFAULTS.tear_position
    : Number(value('tear_position'));
  const lengths = { ring: cut, fringe: 1 - cut };
  const skirts: number[] = [];
  const flare = add(CURVE, { curve: value('flare') as number[], min: 0, max: 1 });
  for (const [part, share] of Object.entries(lengths)) {
    if (share < MIN_PART) continue;
    const length = round(tornLength * share * (1 - RECOIL));
    const skirtProps: NonNullable<SerializedNode['props']> = tearWired ? {} : { length };
    if (!wired.has('thickness')) skirtProps.thickness = Number(value('thickness'));
    if (props.seed !== undefined) skirtProps.seed = props.seed;
    const skirt = add(SKIRT, skirtProps);
    skirts.push(skirt);
    edges.push([flare, 0, skirt, 'flare']);
    if (part === 'ring') edges.push([ringSpot, 0, skirt, 'attachment']);
    else into('a', [skirt, 'attachment']);
    into('thickness', [skirt, 'thickness']);
    into('seed', [skirt, 'seed']);
    if (tearWired) {
      const grows = add(CURVE, { curve: step(false), min: 0, max: length });
      edges.push([grows, 0, skirt, 'length']);
      into('tear', [grows, 'value']);
    }
  }

  if (tearWired) {
    // falls from sticking out to its flare
    const falls = add(CURVE, { curve: [0, JUST_TORN, SWITCH, JUST_TORN, FALLEN, 0, 1, 0] });
    edges.push([falls, 0, flare, 'min']);
    nodes.find((n) => n.id === flare)!.exposed = ['min'];
    into('tear', [falls, 'value']);
    // the membrane is gone once the skirts hang
    delete membrane.props!.thickness;
    const thickness = wired.has('thickness')
      ? add(CURVE, { curve: step(true), min: 0 }, ['max'])
      : add(CURVE, { curve: step(true), min: 0, max: Number(value('thickness')) });
    edges.push([thickness, 0, old.id, 'thickness']);
    into('tear', [thickness, 'value']);
    inputs.thickness = inputs.thickness.filter(([id]) => id !== old.id);
    if (wired.has('thickness')) into('thickness', [thickness, 'max']);
  } else {
    const fallen = Math.min(Math.max((tear - SWITCH) / (FALLEN - SWITCH), 0), 1);
    nodes.find((n) => n.id === flare)!.props!.min = round(JUST_TORN * (1 - fallen));
  }

  const join = add(JOIN, {});
  const meshes = tearWired ? [old.id, ...skirts] : skirts;
  meshes.forEach((id, i) => edges.push([id, 0, join, 'abcd'[i]]));
  if (tearWired) {
    return { nodes: [membrane, ...nodes], edges, inputs, outputs: { 0: [join, 0] } };
  }
  // torn all round already, the closed skin is gone
  for (const key of Object.keys(inputs)) inputs[key] = inputs[key].filter(([id]) => id !== old.id);
  return {
    nodes,
    edges: edges.filter((e) => e[2] !== old.id),
    inputs,
    outputs: { 0: [join, 0] }
  };
}

export function membraneSkirts(graph: Graph) {
  const wired = wiredInputs(graph, MEMBRANE);
  return replaceNodes(graph, MEMBRANE, (old, context) => replace(old, context, wired.get(old)));
}
