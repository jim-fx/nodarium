import type { Box, NodeDefinition, NodeInstance } from '@nodarium/runtime';
import { NODE_WIDTH } from './layoutGraph';
import { getNodeHeight } from './nodeHelpers';

/** Where a node is drawn, in graph units */
export function nodeBox(node: NodeInstance, type: NodeDefinition | undefined): Box {
  const x = node.state?.x ?? node.position[0];
  const y = node.state?.y ?? node.position[1];
  // the type is missing when its wasm failed to load
  const height = type ? getNodeHeight(type) : 20;
  return { minX: x, maxX: x + NODE_WIDTH, minY: y, maxY: y + height };
}

export function boxesOverlap(a: Box, b: Box) {
  return a.maxX > b.minX && a.minX < b.maxX && a.maxY > b.minY && a.minY < b.maxY;
}

export function boxContains(box: Box, x: number, y: number) {
  return x > box.minX && x < box.maxX && y > box.minY && y < box.maxY;
}
