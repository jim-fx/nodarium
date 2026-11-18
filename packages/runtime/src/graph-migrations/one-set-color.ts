import type { Graph } from '@nodarium/types';
import { renameInputs, renameNodeType, scopes } from './helpers';

const MYZEL_SET_COLOR = 'max/myzel/set_color';
const SET_COLOR = 'max/plantarium/set_color';

/** The highest depth of set_color, the levels of a mushroom are far fewer */
const EVERY_LEVEL = 10;

/** The default color of the Myzel set_color, the Plantarium one has another */
const CREAM = [240, 236, 228];

/**
 * Up to graph version 18 Myzel had a set_color of its own that put the color next to the paths
 * for hyphae_to_mesh, for every path of the nodes whose paths passed through it. From version
 * 19 on hyphae_to_mesh reads the colors of the points that Plantarium's set_color writes. Its
 * input is called plant, and its depth colors every level, as the Myzel one did. A node without
 * a color of its own keeps the default of the Myzel one.
 */
export function oneSetColor(graph: Graph) {
  for (const { nodes, edges } of scopes(graph)) {
    for (const node of nodes) {
      if (node.type !== MYZEL_SET_COLOR) continue;
      const wired = edges.some((e) => e[2] === node.id && e[3] === 'color');
      const props: NonNullable<typeof node.props> = { ...node.props, depth: EVERY_LEVEL };
      if (!wired && props.color === undefined) props.color = [...CREAM];
      node.props = props;
    }
  }
  renameInputs(graph, MYZEL_SET_COLOR, { paths: 'plant' });
  return renameNodeType(graph, MYZEL_SET_COLOR, SET_COLOR);
}
