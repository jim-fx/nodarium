import type { Graph, SerializedNode } from '@nodarium/types';
import { idAllocator, removeInputs, renameInputs, type Scope, scopes } from './helpers';

const OUTPUT = 'max/myzel/output';
const HYPHAE_TO_MESH = 'max/myzel/hyphae_to_mesh';

/** The Myzel nodes whose first output is paths */
const PATH_MAKERS = new Set(
  [
    'cap',
    'cap_lopsided',
    'cap_open',
    'margin_curl',
    'margin_waves',
    'set_color',
    'set_fibres',
    'set_fill',
    'strands'
  ].map((name) => `max/myzel/${name}`)
);

const SHOW_HYPHAE = 1;
const DEFAULT_BLEND = 3;

/** How far to the right of the paths the output moves for the node that meshes them to fit */
const STEP = 30;

/**
 * Up to graph version 6 the output node took a mesh, paths or a pattern in the input field, and
 * meshed paths itself, controlled by show, blend, resolution_field and cut. From version 7 on its
 * only input is mesh, paths are shown as lines and hyphae_to_mesh makes the surface. Paths of a
 * Myzel node wired into an output that showed the surface (show not set to "hyphae") now go through
 * a new hyphae_to_mesh, with the blend of the output, resolution_field is a graph setting of both
 * and carries over. An output that showed both keeps only the surface, one that cut itself open
 * is no longer cut. Paths that come from a group, or any node not known to make paths, are left
 * as they are and show as lines.
 */
export function outputMesh(graph: Graph) {
  const nextId = idAllocator(graph);
  for (const scope of scopes(graph)) {
    for (const output of scope.nodes.filter((n) => n.type === OUTPUT)) {
      meshPaths(scope, output, nextId);
    }
  }
  removeInputs(graph, OUTPUT, ['show', 'blend', 'resolution_field', 'cut']);
  return renameInputs(graph, OUTPUT, { field: 'mesh' });
}

function meshPaths(scope: Scope, output: SerializedNode, nextId: () => number) {
  if (output.props?.show === SHOW_HYPHAE) return;
  const typeOf = (id: number) => scope.nodes.find((n) => n.id === id)?.type;
  const paths = scope.edges.filter(
    (e) =>
      e[2] === output.id && e[3] === 'field' && e[1] === 0 && PATH_MAKERS.has(typeOf(e[0]) ?? '')
  );
  if (paths.length === 0) return;

  const blend = output.props?.blend;
  const mesh: SerializedNode = {
    id: nextId(),
    type: HYPHAE_TO_MESH as SerializedNode['type'],
    position: [output.position[0], output.position[1]],
    props: typeof blend === 'number' && blend !== DEFAULT_BLEND ? { blend } : {}
  };
  output.position = [output.position[0] + STEP, output.position[1]];
  scope.nodes.push(mesh);
  scope.edges = scope.edges.map((e) => (paths.includes(e) ? [e[0], e[1], mesh.id, 'paths'] : e));
  scope.edges.push([mesh.id, 0, output.id, 'field']);
}
