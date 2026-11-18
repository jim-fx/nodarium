import type { Graph } from '@nodarium/types';
import { renameNodeType } from './helpers';

/** The value nodes by the collection they lived in up to graph version 15 */
const CORE_NODES = {
  plantarium: ['float', 'math', 'random', 'random_int', 'round', 'curve', 'alpha', 'wave', 'vec3'],
  myzel: ['time', 'mix_color']
};

/** From graph version 16 on the value nodes live in a collection of their own, `max/core` */
export function coreCollection(graph: Graph) {
  for (const [collection, nodes] of Object.entries(CORE_NODES)) {
    for (const node of nodes) {
      renameNodeType(graph, `max/${collection}/${node}`, `max/core/${node}`);
    }
  }
  return graph;
}
