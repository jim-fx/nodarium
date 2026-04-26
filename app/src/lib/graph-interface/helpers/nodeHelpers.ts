import type { NodeDefinition, NodeInstance } from '@nodarium/types';

export function getParameterHeight(node: NodeDefinition, inputKey: string) {
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

const nodeHeightCache: Record<string, number> = {};
export function getNodeHeight(node: NodeDefinition) {
  if (!node) {
    console.trace('Node is undefined', node);
  }
  if (node.id in nodeHeightCache) {
    return nodeHeightCache[node.id];
  }
  if (!node?.inputs) {
    return 5;
  }
  let height = 5;

  for (const key in node.inputs) {
    const h = getParameterHeight(node, key) / 10;
    height += h;
  }

  nodeHeightCache[node.id] = height;
  return height;
}
