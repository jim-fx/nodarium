import type { Box } from '@nodarium/types';
import type { Color } from 'three';
import type { Vector3 } from 'three/src/math/Vector3.js';
import Component from './Debug.svelte';
import { debugState } from './debug.svelte';

export function debugPosition(x: number, y: number) {
  debugState.writePoint(x, y);
}

export function debugRect(rect: Box) {
  debugState.writeRect(rect);
}

export function debugLine(points: Vector3[], color?: Color) {
  debugState.writeLine(points, color);
}

export function clear() {
  debugState.clear();
}

export function clearLines() {
  debugState.clearLines();
}

export default Component;
