import type { Graph, SerializedEdge, SerializedNode } from '@nodarium/types';
import { idAllocator, removeInputs, renameInputs, type Scope, scopes } from './helpers';

const HYPHAE_TO_MESH = 'max/myzel/hyphae_to_mesh';
const JOIN_MESHES = 'max/myzel/join_meshes';
const JOIN = 'max/plantarium/join';
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
 * one and those of their path output are dropped, as are those into loose. set_blend is gone,
 * the paths pass through where it was.
 */
export function joinMeshes(graph: Graph) {
  const nextId = idAllocator(graph);
  for (const scope of scopes(graph)) {
    skipSetBlend(scope);
    moveSheetOutputs(scope);
    joinHyphaeMeshes(scope, nextId);
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

/** The mesh sources of a join node that only gives them to hyphae_to_mesh, in its input order */
function joinedMeshes(
  scope: Scope,
  join: SerializedNode,
  target: SerializedNode
): Source[] | undefined {
  const outgoing = scope.edges.filter((e) => e[0] === join.id);
  if (!outgoing.every((e) => e[2] === target.id && e[3] === 'meshes')) return undefined;
  return JOIN_INPUTS.flatMap((input) => sourcesOf(scope, join, input));
}

function joinHyphaeMeshes(scope: Scope, nextId: () => number) {
  for (const hyphae of scope.nodes.filter((n) => n.type === HYPHAE_TO_MESH)) {
    const meshes = sourcesOf(scope, hyphae, 'meshes');
    if (meshes.length === 0) continue;

    const sources: Source[] = [];
    const joins = new Set<number>();
    for (const source of meshes) {
      const node = scope.nodes.find((n) => n.id === source[0]);
      const inner = node?.type === JOIN ? joinedMeshes(scope, node, hyphae) : undefined;
      if (inner) {
        sources.push(...inner);
        joins.add(source[0]);
      } else {
        sources.push(source);
      }
    }
    scope.nodes = scope.nodes.filter((n) => !joins.has(n.id));
    scope.edges = scope.edges.filter(
      (e) => !joins.has(e[0]) && !joins.has(e[2]) && !(e[2] === hyphae.id && e[3] === 'meshes')
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
