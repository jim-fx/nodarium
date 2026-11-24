import { describe, expect, it } from 'vitest';
import { GraphCamera } from '../camera.svelte';
import { maxZoom } from './constants';
import { TouchEventManager } from './touch.events';

function setup() {
  const camera = new GraphCamera();
  camera.position = [0, 0, 4];
  camera.width = 200;
  camera.height = 100;
  const events = new TouchEventManager(camera, () => ({ x: 0, y: 300 }));
  return { camera, events };
}

function touch(...points: [number, number][]) {
  const touches = points.map(([clientX, clientY]) => ({ clientX, clientY }));
  return { touches, target: null } as unknown as TouchEvent;
}

describe('TouchEventManager', () => {
  it('moves the view with one finger', () => {
    const { camera, events } = setup();
    events.handleStart(touch([100, 350]));
    events.handleMove(touch([120, 342]));
    expect(camera.position).toEqual([-5, 2, 4]);
  });

  it('keeps the point between two fingers under them while it zooms', () => {
    const { camera, events } = setup();
    const before = camera.toWorld(50, 20);
    events.handleStart(touch([40, 320], [60, 320]));
    events.handleMove(touch([30, 320], [70, 320]));
    expect(camera.position[2]).toBe(8);
    const after = camera.toWorld(50, 20);
    expect(after[0]).toBeCloseTo(before[0]);
    expect(after[1]).toBeCloseTo(before[1]);

    events.handleMove(touch([0, 320], [20000, 320]));
    expect(camera.position[2]).toBe(maxZoom);
  });

  it('does not jump when one of two fingers is lifted', () => {
    const { camera, events } = setup();
    events.handleStart(touch([40, 320], [80, 320]));
    events.handleStart(touch([80, 320]));
    events.handleMove(touch([80, 320]));
    expect(camera.position).toEqual([0, 0, 4]);
    events.handleStart(touch());
    events.handleMove(touch([100, 320]));
    expect(camera.position).toEqual([0, 0, 4]);
  });
});
