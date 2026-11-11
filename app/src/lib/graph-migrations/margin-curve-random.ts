import type { Graph, SerializedEdge, SerializedNode } from '@nodarium/types';
import {
  type InputSocket,
  type ReplaceContext,
  type Replacement,
  replaceNodes,
  scopes,
  wiredInputs
} from './helpers';

const CURL = 'max/myzel/margin_curl';
const WAVES = 'max/myzel/margin_waves';
const CURVE = 'max/plantarium/curve';
const RANDOM = 'max/plantarium/random';
const MATH = 'max/plantarium/math';

const ADD = 0;
const MULTIPLY = 2;

/** What margin_curl and margin_waves had before graph version 12 when nothing was saved */
const OLD_DEFAULTS = { amount: 0.2, reach: 0.35, amplitude: 0.15, irregularity: 0.3 };

/** How margin_waves spreads a plain amplitude from the centre to the margin */
const PLAIN_WAVES = [0, 0, 0.5, 0.25, 1, 1];

/** How far apart the new nodes are placed, to the left of the margin node */
const STEP = 25;

type Builder = {
  nodes: SerializedNode[];
  edges: SerializedEdge[];
  add(type: string, props: NonNullable<SerializedNode['props']>): number;
};

function builder(old: SerializedNode, nextId: () => number): Builder {
  const [x, y] = old.position;
  const nodes: SerializedNode[] = [];
  let step = 0;
  return {
    nodes,
    edges: [],
    add(type, props) {
      const id = nextId();
      step++;
      nodes.push({
        id,
        position: [x - STEP * step, y + STEP],
        type: type as SerializedNode['type'],
        props
      });
      return id;
    }
  };
}

function kept(old: SerializedNode, keys: string[]): SerializedNode {
  const props = Object.fromEntries(
    Object.entries(old.props ?? {}).filter(([key]) => keys.includes(key))
  );
  const node: SerializedNode = { ...old, props };
  if (old.exposed) node.exposed = old.exposed.filter((key) => keys.includes(key));
  return node;
}

function socketsOf(id: number, keys: string[]): Record<string, InputSocket[]> {
  return Object.fromEntries(keys.map((key) => [key, [[id, key] as InputSocket]]));
}

/**
 * Points of the curve from where the curl starts on, with nine the margin lies within 0.3 % of
 * the cap radius of where smoothstep put it
 */
const CURL_POINTS = 9;

const round = (x: number) => Math.round(x * 1e6) / 1e6;

/** The points of a curve that rises like smoothstep over the outer `reach` of the ray */
function smoothstepCurve(reach: number) {
  const edge = 1 - Math.min(Math.max(reach, 0.001), 1);
  const points = Array.from({ length: CURL_POINTS }, (_, i) => i / (CURL_POINTS - 1)).flatMap(
    (t) => [round(edge + (1 - edge) * t), round(t * t * (3 - 2 * t))]
  );
  return edge > 0 ? [0, 0, ...points] : points;
}

const CURL_INPUTS = ['cap', 'amount'];

/**
 * Up to graph version 11 margin_curl turned every step by amount × smoothstep over the outer
 * reach of the ray. From version 12 on the amount is read along the ray, alpha 0 at the centre to
 * 1 at the margin, as the turn there, and a plain amount spreads over the outer 0.35 as reach did
 * by default. Where that doesn't match, a curve node into the amount draws the old smoothstep: a
 * plain reach other than 0.35, and an amount that comes from a node, which goes into the max of
 * the curve. A reach that came from a node is dropped, it can't be drawn as a curve.
 */
function replaceCurl(
  old: SerializedNode,
  { nextId }: ReplaceContext,
  wired: Set<string> = new Set()
): Replacement {
  const props = old.props ?? {};
  const reach = Number(props.reach ?? OLD_DEFAULTS.reach);
  const curl = kept(old, CURL_INPUTS);
  const inputs = socketsOf(old.id, CURL_INPUTS);
  const b = builder(old, nextId);

  const amountWired = wired.has('amount');
  if (amountWired || (!wired.has('reach') && reach !== OLD_DEFAULTS.reach)) {
    const amount = Number(props.amount ?? OLD_DEFAULTS.amount);
    delete curl.props!.amount;
    const curve = b.add(
      CURVE,
      amountWired
        ? { curve: smoothstepCurve(reach), min: 0 }
        : { curve: smoothstepCurve(reach), min: 0, max: amount }
    );
    if (amountWired) {
      b.nodes[0].exposed = ['max'];
      inputs.amount = [[curve, 'max']];
    }
    b.edges.push([curve, 0, old.id, 'amount']);
  }

  return {
    nodes: [curl, ...b.nodes],
    edges: b.edges,
    inputs,
    outputs: { 0: [old.id, 0] }
  };
}

const WAVES_INPUTS = ['cap', 'waves', 'amplitude', 'uneven_spacing'];

/**
 * Up to graph version 11 margin_waves read its amplitude once for every cap and made the lobes
 * uneven in height by 1 ± irregularity / 2 and in width, both drawn from the seed. From version
 * 12 on the amplitude is read for every lobe, so a random node gives every lobe its own height,
 * and uneven_spacing only varies the width, drawn from the stipe. The irregularity becomes the
 * uneven spacing and a random node of 1 ± irregularity / 2 that multiplies the amplitude, the
 * seed goes. A plain amplitude with it becomes a curve node with the spread of a plain amplitude,
 * its max is the random.
 */
function replaceWaves(
  old: SerializedNode,
  { nextId }: ReplaceContext,
  wired: Set<string> = new Set()
): Replacement {
  const props = old.props ?? {};
  const renamed: SerializedNode = {
    ...old,
    props: Object.fromEntries(
      Object.entries(props).map((
        [key, value]
      ) => [key === 'irregularity' ? 'uneven_spacing' : key, value])
    )
  };
  if (old.exposed) {
    renamed.exposed = old.exposed.map((key) => key === 'irregularity' ? 'uneven_spacing' : key);
  }
  const waves = kept(renamed, WAVES_INPUTS);
  const inputs = socketsOf(old.id, WAVES_INPUTS);
  inputs.irregularity = [[old.id, 'uneven_spacing']];
  const b = builder(old, nextId);

  const irregularity = Number(props.irregularity ?? OLD_DEFAULTS.irregularity);
  const irregularityWired = wired.has('irregularity');
  if (irregularityWired || irregularity > 0) {
    const amplitude = Number(props.amplitude ?? OLD_DEFAULTS.amplitude);
    const amplitudeWired = wired.has('amplitude');
    delete waves.props!.amplitude;

    let scaled: number;
    if (!irregularityWired && !amplitudeWired) {
      scaled = b.add(RANDOM, {
        min: amplitude * (1 - irregularity / 2),
        max: amplitude * (1 + irregularity / 2)
      });
    } else {
      // amplitude × (1 + irregularity × random(-0.5, 0.5))
      let share: number;
      if (irregularityWired) {
        const random = b.add(RANDOM, { min: -0.5, max: 0.5 });
        const spread = b.add(MATH, { op_type: MULTIPLY });
        share = b.add(MATH, { op_type: ADD, b: 1 });
        b.edges.push([random, 0, spread, 'a'], [spread, 0, share, 'a']);
        inputs.irregularity.push([spread, 'b']);
      } else {
        share = b.add(RANDOM, { min: 1 - irregularity / 2, max: 1 + irregularity / 2 });
      }
      scaled = b.add(
        MATH,
        amplitudeWired ? { op_type: MULTIPLY } : { op_type: MULTIPLY, a: amplitude }
      );
      b.edges.push([share, 0, scaled, 'b']);
      if (amplitudeWired) inputs.amplitude = [[scaled, 'a']];
    }

    if (amplitudeWired) {
      b.edges.push([scaled, 0, old.id, 'amplitude']);
    } else {
      // a plain amplitude only waved the outer part, a connected one is read along the ray
      const curve = b.add(CURVE, { curve: PLAIN_WAVES, min: 0 });
      b.edges.push([scaled, 0, curve, 'max'], [curve, 0, old.id, 'amplitude']);
    }
  }

  return {
    nodes: [waves, ...b.nodes],
    edges: b.edges,
    inputs,
    outputs: { 0: [old.id, 0] }
  };
}

/**
 * The random nodes that feed the amplitude were drawn once for every cap, now they would be
 * drawn for every lobe. They draw once per path instead, the path is the stipe of the cap.
 */
function drawOncePerCap(graph: Graph) {
  for (const scope of scopes(graph)) {
    const waves = new Set(scope.nodes.filter((n) => n.type === WAVES).map((n) => n.id));
    const seen = new Set<number>();
    const queue = scope.edges
      .filter((e) => waves.has(e[2]) && e[3] === 'amplitude')
      .map((e) => e[0]);
    while (queue.length > 0) {
      const id = queue.pop()!;
      if (seen.has(id)) continue;
      seen.add(id);
      const node = scope.nodes.find((n) => n.id === id);
      if (node?.type === RANDOM) node.props = { ...node.props, per_path: 1 };
      queue.push(...scope.edges.filter((e) => e[2] === id).map((e) => e[0]));
    }
  }
}

export function marginCurveRandom(graph: Graph) {
  const curlWired = wiredInputs(graph, CURL);
  replaceNodes(graph, CURL, (old, context) => replaceCurl(old, context, curlWired.get(old)));
  drawOncePerCap(graph);
  const wavesWired = wiredInputs(graph, WAVES);
  return replaceNodes(
    graph,
    WAVES,
    (old, context) => replaceWaves(old, context, wavesWired.get(old))
  );
}
