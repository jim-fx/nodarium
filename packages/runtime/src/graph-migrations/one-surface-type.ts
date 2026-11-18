import {
  type Graph,
  GROUP_INSTANCE,
  GROUP_OUTPUT,
  type GroupDefinition,
  groupIdOf,
  groupOutputIndex
} from '@nodarium/types';
import { renameInputs, renameNodeType } from './helpers';

const JOIN = 'max/plantarium/join';
const SET_COLOR = 'max/plantarium/set_color';

/** The node types whose output is a list typed `path` that may carry geometry */
const PATH_LISTS = new Set([JOIN, SET_COLOR]);

/**
 * Up to graph version 14 Myzel surfaces were meshes, a link type of their own that join_meshes
 * joined and set_mesh_color painted. From version 15 on they are geometry: join joins their groups
 * and set_color paints them, the inputs mesh of cap_pattern and sample_mesh are called geometry and
 * the one of the Myzel output input. Group inputs of type mesh take geometry, or the path list of a
 * join, and group outputs of type mesh give geometry, or a path list where a join or set_color
 * feeds them.
 */
export function oneSurfaceType(graph: Graph) {
  renameNodeType(graph, 'max/myzel/join_meshes', JOIN);
  renameInputs(graph, 'max/myzel/set_mesh_color', { mesh: 'plant' });
  renameNodeType(graph, 'max/myzel/set_mesh_color', SET_COLOR);
  renameInputs(graph, 'max/myzel/cap_pattern', { mesh: 'geometry' });
  renameInputs(graph, 'max/myzel/sample_mesh', { mesh: 'geometry' });
  renameInputs(graph, 'max/myzel/output', { mesh: 'input' });

  const groups = new Map((graph.groups ?? []).map((group) => [group.id, group]));
  const resolving = new Set<number>();

  /** The type of output `index` of a group once its mesh outputs are retyped */
  const groupOutputType = (group: GroupDefinition, index: number): string | undefined => {
    const output = Array.isArray(group.outputs) ? group.outputs[index] : undefined;
    if (output?.type !== 'mesh') return output?.type;
    if (resolving.has(group.id)) return 'geometry';
    resolving.add(group.id);
    const outputNode = group.nodes.find((node) => node.type === GROUP_OUTPUT);
    const edge = group.edges.find(
      ([, , to, socket]) => to === outputNode?.id && groupOutputIndex(socket) === index
    );
    const source = edge && group.nodes.find((node) => node.id === edge[0]);
    let type = 'geometry';
    if (source && PATH_LISTS.has(source.type)) type = 'path';
    else if (source?.type === GROUP_INSTANCE) {
      const inner = groups.get(groupIdOf(source) ?? -1);
      if (inner && groupOutputType(inner, edge![1]) === 'path') type = 'path';
    }
    resolving.delete(group.id);
    return type;
  };

  // some old graphs have an object for the outputs of a group
  const outputsOf = (group: GroupDefinition) => Array.isArray(group.outputs) ? group.outputs : [];
  const retyped = [...groups.values()].map((group) =>
    outputsOf(group).map((_, index) => groupOutputType(group, index))
  );
  [...groups.values()].forEach((group, i) => {
    outputsOf(group).forEach((output, index) => output.type = retyped[i][index] ?? output.type);
    for (const input of Object.values(group.inputs ?? {})) {
      if ((input.type as string) !== 'mesh') continue;
      Object.assign(input, { type: 'geometry', accepts: ['path'] });
    }
  });
  return graph;
}
