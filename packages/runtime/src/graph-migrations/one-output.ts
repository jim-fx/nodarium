import {
  DEBUG_NODE,
  type Graph,
  GROUP_INSTANCE,
  groupIdOf,
  type SerializedNode
} from '@nodarium/types';
import { idAllocator, renameNodeType, scopes } from './helpers';

const OLD_OUTPUTS = ['max/plantarium/output', 'max/myzel/output'];
const OUTPUT = 'max/core/output';
const TO_GEOMETRY = 'max/plantarium/to_geometry';

/** The node types of graph version 17 whose output gives geometry */
const GEOMETRY_NODES = new Set([
  'max/myzel/cap_pattern',
  'max/myzel/hyphae_to_mesh',
  'max/myzel/lamellae',
  'max/myzel/membrane',
  'max/myzel/mesh',
  'max/myzel/pores',
  'max/myzel/ridges',
  'max/myzel/skirt',
  'max/plantarium/box',
  'max/plantarium/deform',
  'max/plantarium/leaf',
  'max/plantarium/leaf_growth',
  'max/plantarium/leaf_surface',
  'max/plantarium/revolve',
  'max/plantarium/to_geometry',
  'max/plantarium/triangle'
]);

const PATTERN_NODES = new Set(['max/myzel/cracks', 'max/myzel/reaction_diffusion']);

/** How far right the output moves to make room for the to_geometry in front of it */
const SHIFT = 30;

/**
 * Up to graph version 17 there were two output nodes: the Plantarium one drew paths as tubes, the
 * Myzel one paths as lines and a pattern as a disc. From version 18 on there is one,
 * `max/core/output`, and it takes only geometry. Where something else reached an output, a
 * `to_geometry` (without bake, the step before the output) is put in front of it, it draws paths
 * as the Plantarium output did and passes geometry and instances on. A pattern goes to a debug
 * node instead, which shows it as a disc.
 */
export function oneOutput(graph: Graph) {
  const nextId = idAllocator(graph);
  const groups = new Map((graph.groups ?? []).map((group) => [group.id, group]));

  const sends = (node: SerializedNode | undefined, output: number) => {
    if (!node) return 'other';
    if (node.type === GROUP_INSTANCE) {
      const group = groups.get(groupIdOf(node) ?? -1);
      const type = Array.isArray(group?.outputs) ? group.outputs[output]?.type : undefined;
      return type === 'geometry' ? 'geometry' : type === 'pattern' ? 'pattern' : 'other';
    }
    if (GEOMETRY_NODES.has(node.type)) return 'geometry';
    return PATTERN_NODES.has(node.type) ? 'pattern' : 'other';
  };

  for (const scope of scopes(graph)) {
    for (const output of scope.nodes.filter((node) => OLD_OUTPUTS.includes(node.type))) {
      const { resolution_circle, ...props } = output.props ?? {};
      if (output.props) output.props = props;
      const edge = scope.edges.find(([, , to, input]) => to === output.id && input === 'input');
      if (!edge) continue;
      const source = scope.nodes.find((node) => node.id === edge[0]);
      const kind = sends(source, edge[1]);
      if (kind === 'geometry') continue;

      const [x, y] = output.position ?? [0, 0];
      const id = nextId();
      // next to the output and its edge, so a file changes in one place
      const after = scope.nodes.indexOf(output) + 1;
      if (kind === 'pattern') {
        scope.nodes.splice(after, 0, { id, position: [x, y + 60], type: DEBUG_NODE, props: {} });
        edge[2] = id;
        continue;
      }
      scope.nodes.splice(after, 0, {
        id,
        position: [x, y],
        type: TO_GEOMETRY,
        props: resolution_circle === undefined ? {} : { resolution_circle }
      } as SerializedNode);
      if (output.position) output.position = [Math.round((x + SHIFT) * 100) / 100, y];
      edge[2] = id;
      scope.edges.splice(scope.edges.indexOf(edge) + 1, 0, [id, 0, output.id, 'input']);
    }
  }
  for (const type of OLD_OUTPUTS) renameNodeType(graph, type, OUTPUT);
  return graph;
}
