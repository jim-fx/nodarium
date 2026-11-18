import type { Graph } from '@nodarium/types';
import { scopes } from './helpers';

/**
 * From graph version 17 on `to_geometry` is the step before the output and passes instances on,
 * only with `bake` it bakes them into its geometry as every `to_geometry` did before
 */
export function toGeometryBake(graph: Graph) {
  for (const { nodes } of scopes(graph)) {
    for (const node of nodes) {
      if (node.type !== 'max/plantarium/to_geometry') continue;
      node.props = { ...node.props, bake: 1 };
    }
  }
  return graph;
}
