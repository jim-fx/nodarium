import type { NodeDefinition, NodeInstance } from '@nodarium/types';

export function getParameterHeight(node: NodeDefinition, inputKey: string) {
  const input = node.inputs?.[inputKey];
  if (!input) {
    if (inputKey.startsWith('__virtual')) {
      return 50;
    }
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

export function getSocketPosition(
  node: NodeInstance,
  index: string | number
): [number, number] {
  if (typeof index === 'number') {
    if (node.type === '__virtual/group/input') {
      const nodeType = node.state.type;
      const keys = Object.keys(nodeType?.inputs || {});
      let height = 5;
      for (let i = 0; i < keys.length; i++) {
        const h = getParameterHeight(nodeType!, keys[i]) / 10;
        if (i === index) { height += h / 2; break; }
        height += h;
      }
      return [
        (node?.state?.x ?? node.position[0]) + 20,
        (node?.state?.y ?? node.position[1]) + height
      ];
    }
    return [
      (node?.state?.x ?? node.position[0]) + 20,
      (node?.state?.y ?? node.position[1]) + 2.5 + 10 * index
    ];
  } else {
    let height = 5;
    const nodeType = node.state.type!;
    const inputs = nodeType.inputs || {};
    for (const inputKey in inputs) {
      const h = getParameterHeight(nodeType, inputKey) / 10;
      if (inputKey === index) {
        height += h / 2;
        break;
      }
      height += h;
    }
    return [
      node?.state?.x ?? node.position[0],
      (node?.state?.y ?? node.position[1]) + height
    ];
  }
}

const nodeHeightCache: Record<string, number> = {};
export function getNodeHeight(node: NodeDefinition) {
  // Don't cache virtual nodes — their inputs can change dynamically
  const isVirtual = (node.id as string).startsWith('__virtual/');
  if (!isVirtual && node.id in nodeHeightCache) {
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

  if (!isVirtual) {
    nodeHeightCache[node.id] = height;
  }
  return height;
}
