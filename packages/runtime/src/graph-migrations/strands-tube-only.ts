import type { Graph, SerializedEdge, SerializedNode } from '@nodarium/types';
import {
  type InputSocket,
  removeInputs,
  type ReplaceContext,
  type Replacement,
  replaceNodes,
  wiredInputs
} from './helpers';
import { STRANDS } from './strands-thickness';

const TWIST = 'max/plantarium/twist';
const RANDOM_BREAK = 'max/plantarium/random_break';
const MATH = 'max/plantarium/math';

const REMOVED = ['twist', 'length_variation'];

/** Inputs strands still has */
const KEPT = ['plant', 'amount', 'thickness', 'spread', 'wobble', 'seed', 'creator'];

const SUBTRACT = 1;
const MULTIPLY = 2;

/** What strands did at most to either end of a strand at a length variation of 1, of the guide */
const MAX_SHORTENING = 0.5;

/**
 * Strands broke off with a chance of 1/2 to keep the mean shortening it had: it took
 * length_variation * 1/2 * random^3 off the tip of every strand, a mean of length_variation / 8
 */
const BREAK_CHANCE = 0.5;

/** How far apart the new nodes are placed, to the right of strands */
const STEP = 25;

/**
 * Up to graph version 9 strands also turned its bundle (twist) and let strands end short of the
 * tip and start above the base (length_variation). From version 10 on a twist node turns the
 * strands around the line they run along, and a random_break behind it ends some of them short
 * of the tip. strands -> twist -> random_break, each one only where it is needed.
 *
 * A plain length_variation becomes the lowest break of random_break, 1 - length_variation / 2,
 * a wired one gets the same sum from math nodes. Strands no longer start above the base.
 */
function replace(
  old: SerializedNode,
  { nextId }: ReplaceContext,
  wired: Set<string> = new Set()
): Replacement | undefined {
  const props = old.props ?? {};
  const twist = Number(props.twist ?? 0);
  const variation = Number(props.length_variation ?? 0);
  const twistWired = wired.has('twist');
  const variationWired = wired.has('length_variation');
  const twisted = twistWired || twist !== 0;
  const varied = variationWired || variation > 0;
  if (!twisted && !varied) return undefined;

  const [x, y] = old.position;
  let step = 0;
  const add = (type: string, nodeProps: NonNullable<SerializedNode['props']>) => {
    const id = nextId();
    nodes.push({
      id,
      type: type as SerializedNode['type'],
      position: [x + STEP * ++step, y],
      props: nodeProps
    });
    return id;
  };

  const strands: SerializedNode = {
    ...old,
    props: Object.fromEntries(Object.entries(props).filter(([key]) => !REMOVED.includes(key)))
  };
  const nodes: SerializedNode[] = [strands];
  const edges: SerializedEdge[] = [];
  const inputs: Record<string, InputSocket[]> = Object.fromEntries(
    KEPT.map((key) => [key, [[old.id, key] as InputSocket]])
  );

  let last = old.id;
  const append = (id: number) => {
    edges.push([last, 0, id, 'plant']);
    last = id;
  };

  if (twisted) {
    const id = add(TWIST, { turns: twist });
    append(id);
    if (twistWired) inputs.twist = [[id, 'turns']];
  }

  if (varied) {
    const id = add(RANDOM_BREAK, {
      chance: BREAK_CHANCE,
      lowest_break: 1 - MAX_SHORTENING * variation,
      highest_break: 1
    });
    append(id);
    if (variationWired) {
      // 1 - length_variation * 1/2
      const shortening = add(MATH, { op_type: MULTIPLY, a: 0, b: MAX_SHORTENING });
      const lowest = add(MATH, { op_type: SUBTRACT, a: 1, b: 0 });
      edges.push([shortening, 0, lowest, 'b'], [lowest, 0, id, 'lowest_break']);
      inputs.length_variation = [[shortening, 'a']];
    }
  }

  return { nodes, edges, inputs, outputs: { 0: [last, 0] } };
}

export function strandsTubeOnly(graph: Graph) {
  const wired = wiredInputs(graph, STRANDS);
  replaceNodes(graph, STRANDS, (old, context) => replace(old, context, wired.get(old)));
  return removeInputs(graph, STRANDS, REMOVED);
}
