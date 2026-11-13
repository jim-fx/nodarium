import type { Graph, SerializedEdge, SerializedNode } from '@nodarium/types';
import { idAllocator, removeInputs, renameInputs, type Scope, scopes } from './helpers';

const SET_COLOR = 'max/myzel/set_color';
const SET_MESH_COLOR = 'max/myzel/set_mesh_color';

/** How far to the right of set_color the node that paints its mesh sits */
const STEP = 30;

/**
 * Up to graph version 5 set_color had two inputs, plant and mesh, and two outputs, the paths
 * and the painted mesh. From version 6 on set_color only takes paths, in the input paths, and
 * set_mesh_color paints a mesh. A set_color with a mesh link becomes a set_mesh_color, mesh output
 * included, one with both becomes one of each with the same color.
 */
export function setMeshColor(graph: Graph) {
  const nextId = idAllocator(graph);
  for (const scope of scopes(graph)) {
    for (const node of scope.nodes.filter((n) => n.type === SET_COLOR)) {
      splitSetColor(scope, node, nextId);
    }
  }
  removeInputs(graph, SET_COLOR, ['mesh']);
  return renameInputs(graph, SET_COLOR, { plant: 'paths' });
}

function splitSetColor(scope: Scope, node: SerializedNode, nextId: () => number) {
  const wiredInto = (input: string) => scope.edges.some((e) => e[2] === node.id && e[3] === input);
  const wiredFrom = (output: number) =>
    scope.edges.some((e) => e[0] === node.id && e[1] === output);
  const meshUsed = wiredInto('mesh') || wiredFrom(1);
  if (!meshUsed) return;

  if (!wiredInto('plant') && !wiredFrom(0)) {
    node.type = SET_MESH_COLOR as SerializedNode['type'];
    scope.edges = scope.edges.map((
      e
    ) => (e[0] === node.id && e[1] === 1 ? [e[0], 0, e[2], e[3]] : e));
    return;
  }

  const mesh: SerializedNode = {
    id: nextId(),
    type: SET_MESH_COLOR as SerializedNode['type'],
    position: [node.position[0] + STEP, node.position[1]],
    props: node.props?.color === undefined ? {} : { color: structuredClone(node.props.color) }
  };
  const exposed = node.exposed?.filter((input) => input === 'color' || input === 'mesh');
  if (exposed?.length) mesh.exposed = exposed;
  scope.nodes.push(mesh);

  const edges: SerializedEdge[] = [];
  for (const edge of scope.edges) {
    const [from, output, to, input] = edge;
    if (to === node.id && input === 'mesh') {
      edges.push([from, output, mesh.id, 'mesh']);
    } else if (from === node.id && output === 1) {
      edges.push([mesh.id, 0, to, input]);
    } else {
      edges.push(edge);
      if (to === node.id && input === 'color') edges.push([from, output, mesh.id, 'color']);
    }
  }
  scope.edges = edges;
}
