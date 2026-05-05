import type {
  Edge,
  NodeDefinition,
  NodeInstance,
  SerializedEdge,
  SerializedNode
} from '@nodarium/types';

export function getParameterHeight(node: NodeDefinition, inputKey: string) {
  if (node.id === '__internal/group/input') {
    return 50;
  }

  const input = node.inputs?.[inputKey];
  if (!input) {
    return 0;
  }

  if (inputKey === 'seed') return 0;
  if (!node.inputs) return 0;
  if ('setting' in input) return 0;
  if (input.hidden) return 0;

  if (input.type === 'shape' && input.external !== true) {
    return 200;
  }
  if (
    input?.label !== '' && !input.external && input.type !== 'path'
    && input.type !== 'geometry'
  ) {
    return 100;
  }
  return 50;
}

export function serializeNode(node: SerializedNode | NodeInstance): SerializedNode {
  return {
    id: node.id,
    position: [...node.position],
    type: node.type,
    props: node.props
  };
}

export function serializeEdge(edge: Edge): SerializedEdge {
  return [edge[0].id, edge[1], edge[2].id, edge[3]];
}

const nodeHeightCache: Record<string, number> = {};
export function getNodeHeight(node: NodeDefinition) {
  if (!node || !('inputs' in node)) {
    return 5;
  }
  if (node.id in nodeHeightCache) {
    return nodeHeightCache[node.id];
  }
  let height = 5;

  for (const key in node.inputs) {
    const h = getParameterHeight(node, key) / 10;
    height += h;
  }

  nodeHeightCache[node.id] = height;
  return height;
}

export function areSocketsCompatible(
  output: string | undefined,
  inputs: string | (string | undefined)[] | undefined
) {
  if (output === '*') return true;
  if (Array.isArray(inputs) && output) {
    return inputs.includes('*') || inputs.includes(output);
  }
  return inputs === output;
}

export function areEdgesEqual(firstEdge: Edge, secondEdge: Edge) {
  if (firstEdge[0].id !== secondEdge[0].id) {
    return false;
  }

  if (firstEdge[1] !== secondEdge[1]) {
    return false;
  }

  if (firstEdge[2].id !== secondEdge[2].id) {
    return false;
  }

  if (firstEdge[3] !== secondEdge[3]) {
    return false;
  }

  return true;
}
