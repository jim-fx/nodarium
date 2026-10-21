import type { Box } from '@nodarium/types';
import type { Color } from 'three';
import { Vector3 } from 'three/src/math/Vector3.js';

export type DebugLine = {
  points: Vector3[];
  color?: Color;
};

class DebugState {
  points = $state.raw<Vector3[]>([]);
  rects = $state.raw<Box[]>([]);
  lines = $state.raw<DebugLine[]>([]);

  writePoint(x: number, y: number) {
    this.points = [...this.points, new Vector3(x, 1, y)];
  }

  writeRect(rect: Box) {
    this.rects = [...this.rects, rect];
  }

  writeLine(points: Vector3[], color?: Color) {
    this.lines = [...this.lines, { points, color }];
  }

  clearLines() {
    this.lines = [];
  }

  clear() {
    this.points = [];
    this.rects = [];
    this.lines = [];
  }
}

export const debugState = new DebugState();
