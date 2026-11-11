import type { Graph, SerializedEdge, SerializedNode } from '@nodarium/types';
import {
  type InputSocket,
  type ReplaceContext,
  type Replacement,
  replaceNodes,
  wiredInputs
} from './helpers';

const LAMELLAE = 'max/myzel/lamellae';
const RIDGES = 'max/myzel/ridges';
const CURVE = 'max/plantarium/curve';

/** What lamellae had before graph version 11 when nothing was saved */
const OLD_DEFAULTS = {
  depth: 0.004,
  depth_curve: [0, 0.3, 0.2, 0.85, 0.45, 1, 0.8, 0.65, 1, 0.1],
  decurrent: 0,
  thickness: 0.0003,
  blunt: 0,
  edge_wave: 0.3
};

const LAMELLAE_INPUTS = [
  'cap',
  'pattern',
  'depth',
  'free_distance',
  'decurrent',
  'thickness',
  'edge_wave',
  'seed',
  'creator'
];
const RIDGES_INPUTS = LAMELLAE_INPUTS.filter((key) => key !== 'free_distance');

/** Where the curve goes, left of the gills */
const OFFSET: [number, number] = [-25, 15];

/**
 * Up to graph version 10 lamellae shaped its depth with depth_curve from the stipe to the margin
 * and made blunt folds with blunt. From version 11 on the depth is read along the gill, alpha 0
 * at the stipe to 1 at the margin, so a curve node into it shapes it, and the folds of
 * Cantharellus are a node of their own, ridges.
 *
 * The depth curve becomes a curve node into the depth that draws it up to the old depth. A depth
 * that came from another node goes into the max of the curve, which multiplies it. A lamellae
 * with blunt above 0.5 becomes ridges, which has no free distance and other defaults, so it gets
 * the old values it relied on.
 */
function replace(
  old: SerializedNode,
  { nextId }: ReplaceContext,
  wired: Set<string> = new Set()
): Replacement {
  const props = old.props ?? {};
  const blunt = Number(props.blunt ?? OLD_DEFAULTS.blunt);
  const ridges = blunt > 0.5;
  const keys = ridges ? RIDGES_INPUTS : LAMELLAE_INPUTS;

  const kept = Object.fromEntries(Object.entries(props).filter(([key]) => keys.includes(key)));
  if (ridges) {
    for (const key of ['decurrent', 'thickness', 'edge_wave'] as const) {
      if (!wired.has(key)) kept[key] ??= OLD_DEFAULTS[key];
    }
  }
  const gills: SerializedNode = {
    ...old,
    type: (ridges ? RIDGES : LAMELLAE) as SerializedNode['type'],
    props: kept
  };
  if (old.exposed) gills.exposed = old.exposed.filter((key) => keys.includes(key));

  const inputs: Record<string, InputSocket[]> = Object.fromEntries(
    keys.map((key) => [key, [[old.id, key] as InputSocket]])
  );
  const nodes: SerializedNode[] = [gills];
  const edges: SerializedEdge[] = [];

  const shape = (props.depth_curve ?? OLD_DEFAULTS.depth_curve) as number[];
  const flat = shape.filter((_, i) => i % 2 === 1).every((y) => y === 1);
  if (!flat) {
    delete kept.depth;
    const curve: SerializedNode = {
      id: nextId(),
      position: [old.position[0] + OFFSET[0], old.position[1] + OFFSET[1]],
      type: CURVE as SerializedNode['type'],
      props: wired.has('depth')
        ? { curve: shape, min: 0 }
        : { curve: shape, min: 0, max: Number(props.depth ?? OLD_DEFAULTS.depth) }
    };
    if (wired.has('depth')) {
      curve.exposed = ['max'];
      inputs.depth = [[curve.id, 'max']];
    }
    nodes.push(curve);
    edges.push([curve.id, 0, old.id, 'depth']);
  }

  return { nodes, edges, inputs, outputs: { 0: [old.id, 0] } };
}

export function lamellaeRidges(graph: Graph) {
  const wired = wiredInputs(graph, LAMELLAE);
  return replaceNodes(graph, LAMELLAE, (old, context) => replace(old, context, wired.get(old)));
}
