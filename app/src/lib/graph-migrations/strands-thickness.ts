import type { SerializedEdge, SerializedNode } from '@nodarium/types';
import type { InputSocket, ReplaceContext, Replacement } from './helpers';

export const STRANDS = 'max/myzel/strands';

/** The thickness of max/myzel/strands before graph version 4, its default then */
const OLD_THICKNESS = 0.0005;

/** Inputs strands still has */
const KEPT = [
  'plant',
  'amount',
  'thickness',
  'spread',
  'twist',
  'wobble',
  'length_variation',
  'seed',
  'creator'
];

const MULTIPLY = 2;
const SUBTRACT = 1;

/** How far apart the new nodes are placed, to the left of strands */
const STEP = 25;

/**
 * Up to graph version 3 strands thinned every strand by a random share of up to
 * thickness_variation itself, thickness * (1 - variation * random) with a uniform random.
 * From version 4 on a random node wired into the thickness does that, it is drawn for every
 * strand. A plain variation becomes a random node from the thinnest to the full thickness, a
 * wired thickness is multiplied by a random node, and a wired variation gets the whole formula
 * from math nodes. Strands without a variation only lose the input, see removeInputs().
 */
export function strandsThickness(
  old: SerializedNode,
  { nextId }: ReplaceContext,
  wired: Set<string> = new Set()
): Replacement | undefined {
  const props = old.props ?? {};
  const variation = Number(props.thickness_variation ?? 0);
  const variationWired = wired.has('thickness_variation');
  if (!variationWired && !(variation > 0)) return undefined;

  const thickness = Number(props.thickness ?? OLD_THICKNESS);
  const thicknessWired = wired.has('thickness');
  const [x, y] = old.position;
  let step = 0;
  const place = (): [number, number] => [x - STEP * ++step, y + STEP];

  const strands: SerializedNode = {
    ...old,
    props: Object.fromEntries(
      Object.entries(props).filter(([key]) => key !== 'thickness_variation')
    )
  };
  const nodes: SerializedNode[] = [strands];
  const edges: SerializedEdge[] = [];
  const inputs: Record<string, InputSocket[]> = Object.fromEntries(
    KEPT.map((key) => [key, [[old.id, key] as InputSocket]])
  );
  const add = (type: string, nodeProps: NonNullable<SerializedNode['props']>) => {
    const id = nextId();
    nodes.push({ id, type: type as SerializedNode['type'], position: place(), props: nodeProps });
    return id;
  };
  const random = (min: number, max: number) =>
    add('max/plantarium/random', { min, max, uniform: 1 });
  const math = (op: number, a: number, b: number) =>
    add('max/plantarium/math', { op_type: op, a, b });

  if (!variationWired && !thicknessWired) {
    edges.push([random(thickness * (1 - variation), thickness), 0, old.id, 'thickness']);
  } else if (!variationWired) {
    const share = random(1 - variation, 1);
    const product = math(MULTIPLY, thickness, 1);
    edges.push([share, 0, product, 'b'], [product, 0, old.id, 'thickness']);
    inputs.thickness = [[product, 'a']];
  } else {
    // thickness * (1 - variation * random)
    const share = random(0, 1);
    const thinner = math(MULTIPLY, variation, 1);
    const kept = math(SUBTRACT, 1, 0);
    const product = math(MULTIPLY, thickness, 1);
    edges.push(
      [share, 0, thinner, 'b'],
      [thinner, 0, kept, 'b'],
      [kept, 0, product, 'b'],
      [product, 0, old.id, 'thickness']
    );
    inputs.thickness_variation = [[thinner, 'a']];
    if (thicknessWired) inputs.thickness = [[product, 'a']];
  }

  return { nodes, edges, inputs, outputs: { 0: [old.id, 0] } };
}
