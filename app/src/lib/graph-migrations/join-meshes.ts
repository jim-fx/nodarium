import type { Graph, SerializedEdge, SerializedNode } from '@nodarium/types';
import { idAllocator, removeInputs, renameInputs, type Scope, scopes } from './helpers';

const HYPHAE_TO_MESH = 'max/myzel/hyphae_to_mesh';
const JOIN_MESHES = 'max/myzel/join_meshes';
const JOIN = 'max/plantarium/join';
const SET_COLOR = 'max/myzel/set_color';
const SET_BLEND = 'max/myzel/set_blend';
const SHEETS = ['max/myzel/lamellae', 'max/myzel/membrane'];

const JOIN_INPUTS = ['a', 'b', 'c', 'd'];

/** How far to the right of hyphae_to_mesh the node that joins its mesh with the others sits */
const STEP = 30;

type Source = [nodeId: number, output: number];

/**
 * Up to graph version 4 hyphae_to_mesh took finished meshes through its meshes input, a list
 * of them from a join node, and its paths in two inputs, hyphae and loose. From version 5 on it
 * takes paths in one input, paths, and join_meshes joins its mesh with the others. lamellae and
 * membrane give their mesh as the only output, so the edges of their mesh output move up by
 * one and those of their path output are dropped, as are those into loose. A sheet whose path
 * output went into hyphae_to_mesh, directly or through join nodes, gets its mesh joined instead,
 * so the gills and the veil stay in the mesh. set_blend is gone, the paths pass through where
 * it was.
 */
export function joinMeshes(graph: Graph) {
  const nextId = idAllocator(graph);
  for (const scope of scopes(graph)) {
    skipSetBlend(scope);
    const strays = straySheets(scope);
    moveSheetOutputs(scope);
    joinHyphaeMeshes(scope, nextId, strays);
  }
  renameInputs(graph, HYPHAE_TO_MESH, { hyphae: 'paths' });
  removeInputs(graph, HYPHAE_TO_MESH, ['loose', 'meshes']);
  removeInputs(graph, 'max/myzel/membrane', ['strands']);
  return graph;
}

function skipSetBlend(scope: Scope) {
  const blends = new Set(scope.nodes.filter((n) => n.type === SET_BLEND).map((n) => n.id));
  if (blends.size === 0) return;
  // what is wired into the paths of a set_blend node, through any set_blend nodes before it
  const behind = (id: number): Source[] =>
    scope.edges
      .filter((e) => e[2] === id && e[3] === 'plant')
      .flatMap((e): Source[] => (blends.has(e[0]) ? behind(e[0]) : [[e[0], e[1]]]));
  const edges: SerializedEdge[] = [];
  for (const edge of scope.edges) {
    if (blends.has(edge[2])) continue;
    if (blends.has(edge[0])) {
      for (const [from, output] of behind(edge[0])) edges.push([from, output, edge[2], edge[3]]);
    } else {
      edges.push(edge);
    }
  }
  scope.edges = edges;
  scope.nodes = scope.nodes.filter((n) => !blends.has(n.id));
}

/** The sheets whose path output went into hyphae_to_mesh, through join nodes or not, by hyphae_to_mesh */
function straySheets(scope: Scope) {
  const strays = new Map<number, Source[]>();
  const sheets = new Set(scope.nodes.filter((n) => SHEETS.includes(n.type)).map((n) => n.id));
  const typeOf = (id: number) => scope.nodes.find((n) => n.id === id)?.type;
  const reach = (id: number, seen: Set<number>): number[] => {
    if (seen.has(id)) return [];
    seen.add(id);
    if (typeOf(id) === HYPHAE_TO_MESH) return [id];
    if (typeOf(id) !== JOIN) return [];
    return scope.edges.filter((e) => e[0] === id).flatMap((e) => reach(e[2], seen));
  };
  for (const [from, output, to] of scope.edges) {
    if (!sheets.has(from) || output !== 0) continue;
    for (const hyphae of reach(to, new Set())) {
      strays.set(hyphae, [...(strays.get(hyphae) ?? []), [from, 0]]);
    }
  }
  return strays;
}

function moveSheetOutputs(scope: Scope) {
  const sheets = new Set(scope.nodes.filter((n) => SHEETS.includes(n.type)).map((n) => n.id));
  scope.edges = scope.edges
    .filter(([from, output]) => !(sheets.has(from) && output === 0))
    .map((edge) => (sheets.has(edge[0]) ? [edge[0], 0, edge[2], edge[3]] : edge));
}

/** The nodes behind the edges into `input` of `node`, in the order of the edges */
function sourcesOf(scope: Scope, node: SerializedNode, input: string): Source[] {
  return scope.edges.filter((e) => e[2] === node.id && e[3] === input).map((e) => [e[0], e[1]]);
}

/** Whether a source gives a mesh, after the outputs of the sheets moved up */
function isMesh(scope: Scope, [id, output]: Source) {
  const type = scope.nodes.find((n) => n.id === id)?.type;
  if (type === undefined) return false;
  if (type === SET_COLOR) return output === 1;
  return SHEETS.includes(type) || type === HYPHAE_TO_MESH || type === JOIN_MESHES;
}

/**
 * The mesh sources of a join node that gives them to the meshes input of `target`, in its input
 * order, and the edges to leave out. A join that has other consumers stays and only loses the
 * edges of its meshes, a join that has none goes with all its edges.
 */
function joinedMeshes(scope: Scope, join: SerializedNode, target: SerializedNode) {
  const into = scope.edges.filter((e) => e[2] === join.id && JOIN_INPUTS.includes(e[3]));
  const ordered = JOIN_INPUTS.flatMap((input) => into.filter((e) => e[3] === input));
  const shared = scope.edges.some(
    (e) => e[0] === join.id && !(e[2] === target.id && e[3] === 'meshes')
  );
  if (!shared) {
    return { sources: ordered.map((e): Source => [e[0], e[1]]), drop: ordered, remove: true };
  }
  const meshes = ordered.filter((e) => isMesh(scope, [e[0], e[1]]));
  for (const edge of ordered) {
    if (!scope.nodes.some((n) => n.id === edge[0])) {
      console.warn(
        `join_meshes migration: the join ${join.id} also feeds other nodes and its input ${
          edge[3]
        } comes from outside this graph, it stays in the join`
      );
    }
  }
  return { sources: meshes.map((e): Source => [e[0], e[1]]), drop: meshes, remove: false };
}

function joinHyphaeMeshes(scope: Scope, nextId: () => number, strays: Map<number, Source[]>) {
  for (const hyphae of scope.nodes.filter((n) => n.type === HYPHAE_TO_MESH)) {
    const meshes = sourcesOf(scope, hyphae, 'meshes');
    const stray = strays.get(hyphae.id) ?? [];
    if (meshes.length === 0 && stray.length === 0) continue;

    const sources: Source[] = [];
    const dropped = new Set<SerializedEdge>();
    const removed = new Set<number>();
    for (const source of meshes) {
      const node = scope.nodes.find((n) => n.id === source[0]);
      if (node?.type === JOIN) {
        const inner = joinedMeshes(scope, node, hyphae);
        sources.push(...inner.sources);
        inner.drop.forEach((e) => dropped.add(e));
        if (inner.remove) removed.add(node.id);
      } else {
        sources.push(source);
      }
    }
    for (const source of stray) {
      if (!sources.some(([id, output]) => id === source[0] && output === source[1])) {
        sources.push(source);
      }
    }
    scope.nodes = scope.nodes.filter((n) => !removed.has(n.id));
    scope.edges = scope.edges.filter(
      (e) =>
        !dropped.has(e) && !removed.has(e[0]) && !removed.has(e[2])
        && !(e[2] === hyphae.id && e[3] === 'meshes')
    );

    // what was wired to the mesh of hyphae_to_mesh gets the joined one
    const outgoing = scope.edges.filter((e) => e[0] === hyphae.id);
    scope.edges = scope.edges.filter((e) => e[0] !== hyphae.id);

    // the groups of the paths come first, then those of the meshes one after the other. A join
    // has four inputs, the next one in a row takes the one before it as its first
    let last: Source = [hyphae.id, 0];
    const [x, y] = hyphae.position;
    for (let rest = sources, row = 1; rest.length > 0; row++) {
      const join: SerializedNode = {
        id: nextId(),
        type: JOIN_MESHES as SerializedNode['type'],
        position: [x + STEP * row, y],
        props: {}
      };
      scope.nodes.push(join);
      [last, ...rest.slice(0, 3)].forEach(([from, output], i) => {
        scope.edges.push([from, output, join.id, JOIN_INPUTS[i]]);
      });
      rest = rest.slice(3);
      last = [join.id, 0];
    }
    for (const [, , to, input] of outgoing) scope.edges.push([last[0], last[1], to, input]);
  }
}
