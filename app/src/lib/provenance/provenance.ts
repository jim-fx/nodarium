import {
  HIGHLIGHT,
  type HighlightMode,
  type PartHighlight
} from '#lib/result-viewer/provenance/highlight.ts';
import { expandWithChains, toCreatorId } from '@nodarium/runtime';
import type { Graph } from '@nodarium/types';
import { pathLineage, type PathLink } from '@nodarium/utils';

/**
 * Which node made which part of the result. A path and a tissue of a Myzel mesh carry the creator
 * id of the node that made them, the index leads from that id back to the node as the editor
 * shows it, also through groups.
 */
export type ProvenanceIndex = {
  /** by the id a node runs under: the group nodes to enter from the root, then the node itself */
  chains: Map<number, number[]>;
  types: Map<number, string>;
  /** by creator id, the id the node runs under */
  creators: Map<number, number>;
  /** the nodes that make parts of their own: they get a creator id and send paths or a mesh */
  makers: Set<number>;
  /** the nodes whose first output sends paths, points or a mesh */
  sendsParts: Set<number>;
  /** by the id a node runs under, the nodes whose parts come into the input it works on */
  mainSources: Map<number, number[]>;
  /** by the id a node runs under, the nodes that read any of its outputs */
  readers: Map<number, Reader[]>;
};

/**
 * A node that reads an output of another. `main` when parts arrive at the input the reader works
 * on (the plant of gravity, the cap of lamellae), `side` when they arrive at an input it only
 * looks at (what separate compares with), `value` for everything that is no part: a number, a
 * curve, a pattern.
 */
type Reader = { node: number; flow: 'main' | 'side' | 'value' };

/** What a result knows of where its parts come from */
export type ResultParts = {
  /** the links of its paths, by path id */
  links: Map<number, PathLink>;
  /** the creator ids of the nodes that made a tissue of its Myzel meshes */
  tissues: Set<number>;
};

/**
 * Parts of a result: the geometry of paths and what sits on paths, both by path id, and the
 * tissues of Myzel meshes by the creator id of the node that made them
 */
export type Parts = { paths: Set<number>; instances: Set<number>; creators: Set<number> };

const INSTANCE_TYPES = /\/instance(_from_points)?$/;
const OUTPUT_TYPES = /\/output$/;

function push<K, V>(map: Map<K, V[]>, key: K, value: V) {
  const list = map.get(key);
  if (list) list.push(value);
  else map.set(key, [value]);
}

const noParts = (): Parts => ({ paths: new Set(), instances: new Set(), creators: new Set() });

export function isEmpty(parts: Parts) {
  return !parts.paths.size && !parts.instances.size && !parts.creators.size;
}

/** What the index reads of the definition of a node type */
export type NodeTraits = {
  /** whether an output sends paths, points or a mesh, the things parts are made of */
  isPartOutput: (type: string, output: number) => boolean;
  /** whether `input` is the one the node works on, not one it only looks at */
  isMainInput: (type: string, input: string) => boolean;
  /** whether the runtime gives the node a creator id */
  hasCreator: (type: string) => boolean;
};

export function buildIndex(graph: Graph, traits: NodeTraits): ProvenanceIndex {
  const { graph: expanded, chains } = expandWithChains(graph);
  const types = new Map(expanded.nodes.map((node) => [node.id, node.type]));
  const creators = new Map<number, number>();
  const makers = new Set<number>();
  const sendsParts = new Set<number>();
  for (const node of expanded.nodes) {
    const creator = toCreatorId(node.id);
    if (!creators.has(creator)) creators.set(creator, node.id);
    if (!traits.isPartOutput(node.type, 0)) continue;
    sendsParts.add(node.id);
    // `random` gets a creator id too, but only sends a number
    if (traits.hasCreator(node.type)) makers.add(node.id);
  }
  const mainSources = new Map<number, number[]>();
  const readers = new Map<number, Reader[]>();
  for (const [from, output, to, input] of expanded.edges) {
    const type = types.get(from);
    const part = !!type && traits.isPartOutput(type, output);
    const main = traits.isMainInput(types.get(to) ?? '', input);
    if (part && main) push(mainSources, to, from);
    push(readers, from, { node: to, flow: part ? (main ? 'main' : 'side') : 'value' });
  }
  return { chains, types, creators, makers, sendsParts, mainSources, readers };
}

function startsWith(chain: number[], start: number[]) {
  return start.length <= chain.length && start.every((id, i) => chain[i] === id);
}

/**
 * The nodes that run for a node of the editor: itself, or everything inside it when it is a group.
 * `entered` are the group nodes entered from the root to get to the level the node is on.
 */
export function runningNodes(index: ProvenanceIndex, entered: number[], nodeId: number) {
  const start = [...entered, nodeId];
  const running: number[] = [];
  for (const [id, chain] of index.chains) if (startsWith(chain, start)) running.push(id);
  return running;
}

/** The node to show for a node that runs, on the level reached by entering `entered` */
export function visibleNode(chain: number[], entered: number[]) {
  return entered.length < chain.length && startsWith(chain, entered)
    ? chain[entered.length]
    : undefined;
}

const byCreatorCache = new WeakMap<Map<number, PathLink>, Map<number, number[]>>();

function pathsByCreator(links: Map<number, PathLink>) {
  let byCreator = byCreatorCache.get(links);
  if (!byCreator) {
    byCreator = new Map();
    for (const link of links.values()) push(byCreator, link.creator, link.id);
    byCreatorCache.set(links, byCreator);
  }
  return byCreator;
}

function upstream(index: ProvenanceIndex, from: number[]) {
  const seen = new Set(from);
  const queue = [...from];
  for (let i = 0; i < queue.length; i++) {
    for (const source of index.mainSources.get(queue[i]) ?? []) {
      if (seen.has(source)) continue;
      seen.add(source);
      queue.push(source);
    }
  }
  return seen;
}

/** The parts of the result that `nodes` made themselves */
function madeBy(index: ProvenanceIndex, result: ResultParts, nodes: Iterable<number>): Parts {
  const byCreator = pathsByCreator(result.links);
  const parts = noParts();
  for (const id of nodes) {
    // two nodes can fold to one creator id, the parts belong to the first of them
    const creator = toCreatorId(id);
    if (index.creators.get(creator) !== id) continue;
    for (const path of byCreator.get(creator) ?? []) {
      parts.paths.add(path);
      parts.instances.add(path);
    }
    if (result.tissues.has(creator)) parts.creators.add(creator);
  }
  return parts;
}

function merge(into: Parts, from: Parts) {
  for (const id of from.paths) into.paths.add(id);
  for (const id of from.instances) into.instances.add(id);
  for (const id of from.creators) into.creators.add(id);
}

function isInstanceNode(index: ProvenanceIndex, id: number) {
  return INSTANCE_TYPES.test(index.types.get(id) ?? '');
}

/**
 * The parts that come through the nodes `through`, which make none themselves: those of the
 * makers upstream. Of an instance node only what sits on those paths, it does not know its own
 * instances.
 */
function comingThrough(index: ProvenanceIndex, result: ResultParts, through: number[]) {
  const parts = madeBy(index, result, upstream(index, through));
  if (through.every((id) => isInstanceNode(index, id))) parts.paths = new Set();
  return parts;
}

/**
 * The parts made of what `from` sends, for nodes that leave nothing of their own in the result:
 * a curve, a leaf shape, a stem whose paths only carry strands, a cap whose rays only carry
 * tissue and gills. The walk follows the readers and ends at the first that has parts.
 *
 * What is sent are parts (`sendsParts`) or values: a number, a curve, a shape. Parts are not
 * followed into an input that only looks at them, and where they arrive at a maker they end,
 * unless `throughMakers`: so the stem of a mushroom leads to its stipe and not on to the cap that
 * grows on it. A value shapes what its reader does: it leads to the parts that come through a
 * node that passes parts on, and on through a maker that left nothing in the result.
 */
function nearestDownstream(
  index: ProvenanceIndex,
  result: ResultParts,
  from: number[],
  sendsParts: boolean,
  throughMakers: boolean
) {
  const parts = noParts();
  const visited = new Set(from);
  const queue = from.map((node) => ({ node, parts: sendsParts }));
  for (let i = 0; i < queue.length; i++) {
    for (const { node: reader, flow } of index.readers.get(queue[i].node) ?? []) {
      // what was a value once stays one, whatever the sockets it passes are called
      const arriving = queue[i].parts ? flow : 'value';
      if (arriving === 'side' || visited.has(reader)) continue;
      visited.add(reader);
      if (OUTPUT_TYPES.test(index.types.get(reader) ?? '')) continue;
      const next = { node: reader, parts: arriving === 'main' };
      if (index.makers.has(reader)) {
        const made = madeBy(index, result, [reader]);
        if (!isEmpty(made)) merge(parts, made);
        else if (arriving === 'value' || throughMakers) queue.push(next);
        continue;
      }
      const through = arriving === 'value' ? comingThrough(index, result, [reader]) : noParts();
      if (isEmpty(through)) queue.push(next);
      else merge(parts, through);
    }
  }
  return parts;
}

/**
 * What the nodes `running` stand for in the result:
 * - the parts they made, when they made any
 * - else, for nodes that only pass parts on (gravity, set_color, hyphae_to_mesh), the parts that
 *   come through them
 * - else the nearest parts made of what they send, see `nearestDownstream`. Only when the makers
 *   their parts arrive at left nothing in the result either, the walk goes on through those.
 */
export function partsOfRunning(
  index: ProvenanceIndex,
  result: ResultParts,
  running: number[]
): Parts {
  if (running.some((id) => OUTPUT_TYPES.test(index.types.get(id) ?? ''))) return noParts();

  const made = madeBy(index, result, running);
  if (!isEmpty(made)) return made;

  if (!running.some((id) => index.makers.has(id))) {
    const through = comingThrough(index, result, running);
    if (!isEmpty(through)) return through;
  }
  const sendsParts = running.some((id) => index.sendsParts.has(id));
  const nearest = nearestDownstream(index, result, running, sendsParts, false);
  return isEmpty(nearest) ? nearestDownstream(index, result, running, sendsParts, true) : nearest;
}

/**
 * The node that runs and made the part. A tissue of a Myzel mesh names its creator. An instance
 * only knows the path it sits on: it leads to the instance node when exactly one instance node
 * gets the paths of that creator, else to the creator of the path.
 */
export function nodeOfPart(
  index: ProvenanceIndex,
  result: ResultParts,
  hit: { kind: 'geometry' | 'instances' | 'mesh'; pathId: number; creator?: number }
) {
  if (hit.kind === 'mesh') {
    return hit.creator === undefined ? undefined : index.creators.get(hit.creator);
  }
  const link = result.links.get(hit.pathId);
  const creator = link && index.creators.get(link.creator);
  if (creator === undefined) return;
  if (hit.kind !== 'instances') return creator;

  const candidates: number[] = [];
  for (const id of index.types.keys()) {
    if (isInstanceNode(index, id) && upstream(index, [id]).has(creator)) candidates.push(id);
  }
  return candidates.length === 1 ? candidates[0] : creator;
}

/** The parts of several nodes of one level, see `partsOfRunning` */
export function partsOfNodes(
  index: ProvenanceIndex,
  result: ResultParts,
  entered: number[],
  nodeIds: number[]
) {
  const parts = noParts();
  for (const nodeId of nodeIds) {
    merge(parts, partsOfRunning(index, result, runningNodes(index, entered, nodeId)));
  }
  return parts;
}

/** What was picked in the viewer: paths, or the tissues of creators */
export type PickedParts = {
  paths: number[];
  creators: number[];
  /** only what sits on the paths: a leaf that led to its instance node */
  instancesOnly: boolean;
};

/** The picked paths and tissues as parts, with `descendants` also what grows on the paths */
export function partsOfPicked(
  links: Map<number, PathLink>,
  picked: PickedParts,
  descendants = false
): Parts {
  const paths = descendants ? withDescendants(links, picked.paths) : new Set(picked.paths);
  return {
    paths: picked.instancesOnly ? new Set() : paths,
    instances: new Set(paths),
    creators: new Set(picked.creators)
  };
}

/**
 * What the viewer shows of the parts. The parts of the selection are tinted, lighter where a
 * picked part stands out among them. With `isolated` only those parts are shown, as they are.
 */
export function composeHighlight(parts: {
  selected?: Parts;
  picked?: Parts;
  hovered?: Parts;
  isolated?: Parts;
  /** names the selection, see `PartHighlight.selection` */
  selection?: string;
}): PartHighlight | undefined {
  const { selected, picked, hovered, isolated } = parts;
  const paths = new Map<number, HighlightMode>();
  const instances = new Map<number, HighlightMode>();
  const creators = new Map<number, HighlightMode>();
  const paint = (painted: Parts | undefined, mode: HighlightMode) => {
    if (!painted) return;
    for (const id of painted.paths) if (!isolated || isolated.paths.has(id)) paths.set(id, mode);
    for (const id of painted.instances) {
      if (!isolated || isolated.instances.has(id)) instances.set(id, mode);
    }
    for (const id of painted.creators) {
      if (!isolated || isolated.creators.has(id)) creators.set(id, mode);
    }
  };

  if (isolated) {
    paint(isolated, HIGHLIGHT.none);
  } else {
    paint(selected, picked ? HIGHLIGHT.selected : HIGHLIGHT.focus);
    paint(picked, HIGHLIGHT.focus);
  }
  paint(hovered, HIGHLIGHT.hover);

  if (!isolated && !paths.size && !instances.size && !creators.size) return;
  return {
    paths,
    instances,
    creators,
    rest: isolated ? HIGHLIGHT.hidden : HIGHLIGHT.none,
    selection: parts.selection
  };
}

/** The paths that grow on `roots`, directly or on one of their children, and the roots */
export function withDescendants(links: Map<number, PathLink>, roots: Iterable<number>) {
  const children = new Map<number, number[]>();
  for (const link of links.values()) push(children, link.parentId, link.id);
  const all = new Set(roots);
  const queue = [...all];
  for (let i = 0; i < queue.length; i++) {
    for (const child of children.get(queue[i]) ?? []) {
      if (all.has(child)) continue;
      all.add(child);
      queue.push(child);
    }
  }
  return all;
}

/** The path and the paths it grows on, down to the one that grows on its own */
export function lineageOf(links: Map<number, PathLink>, pathId: number) {
  return pathLineage(links.values(), pathId);
}

/** The nodes of one level between `from` and the end of the graph, `from` included */
export function downstream(edges: Iterable<[number, number]>, from: Iterable<number>) {
  const targets = new Map<number, number[]>();
  for (const [source, target] of edges) push(targets, source, target);
  const all = new Set(from);
  const queue = [...all];
  for (let i = 0; i < queue.length; i++) {
    for (const target of targets.get(queue[i]) ?? []) {
      if (all.has(target)) continue;
      all.add(target);
      queue.push(target);
    }
  }
  return all;
}
