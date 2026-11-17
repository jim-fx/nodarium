import { flushSync } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { GraphCamera, persistCamera } from './camera.svelte';
import { maxZoom, minZoom } from './graph/constants';

function createCamera(position: [number, number, number], width = 100, height = 100) {
  const camera = new GraphCamera();
  camera.position = position;
  camera.width = width;
  camera.height = height;
  return camera;
}

const noPadding = { left: 0, right: 0, top: 0, bottom: 0 };

describe('toWorld', () => {
  it('maps the viewport centre to the camera position', () => {
    const [wx, wy] = createCamera([140, 100, 3.5]).toWorld(50, 50);
    expect(wx).toBeCloseTo(140);
    expect(wy).toBeCloseTo(100);
  });

  it('offsets correctly for a point not at centre', () => {
    const [wx, wy] = createCamera([0, 0, 2]).toWorld(100, 50);
    expect(wx).toBeCloseTo(25);
    expect(wy).toBeCloseTo(0);
  });
});

describe('bounds', () => {
  it('spans the view around the camera', () => {
    expect(createCamera([10, 20, 2], 100, 60).bounds).toEqual({
      minX: -15,
      maxX: 35,
      minY: 5,
      maxY: 35
    });
  });
});

describe('snapLevel', () => {
  it('is 1 up to zoom 11 and 2 above', () => {
    const levels = [minZoom, 5, 11, 11.5, 25, maxZoom].map((zoom) =>
      createCamera([0, 0, zoom]).snapLevel()
    );
    expect(levels).toEqual([1, 1, 1, 2, 2, 2]);
  });

  it('takes every level the camera can zoom to', () => {
    const camera = createCamera([0, 0, maxZoom]);
    const levels = new Set<number>();
    for (let i = 0; i < 200; i++) {
      levels.add(camera.snapLevel());
      camera.zoomAt([0, 0], 100);
    }
    expect(camera.position[2]).toBe(minZoom);
    expect([...levels].sort()).toEqual([1, 2]);
  });
});

describe('fit', () => {
  it('centres the box and zooms until its larger side fills the view', () => {
    const camera = createCamera([0, 0, 1], 200, 100);
    const [x, y, zoom] = camera.fit({ minX: 0, maxX: 20, minY: 0, maxY: 20 }, noPadding, maxZoom);
    expect([x, y, zoom]).toEqual([10, 10, 5]);
  });

  it('puts the box in the centre of the area the padding leaves free', () => {
    const camera = createCamera([0, 0, 1], 300, 200);
    const box = { minX: 0, maxX: 40, minY: 0, maxY: 40 };
    const padding = { left: 100, right: 0, top: 0, bottom: 40 };
    camera.position = camera.fit(box, padding, maxZoom);

    expect(camera.position[2]).toBeCloseTo(4);
    const [cx, cy] = camera.toWorld(100 + 200 / 2, 160 / 2);
    expect(cx).toBeCloseTo(20);
    expect(cy).toBeCloseTo(20);
  });

  it('keeps the zoom between the minimum and the given maximum', () => {
    const camera = createCamera([0, 0, 1], 100, 100);
    expect(camera.fit({ minX: 0, maxX: 1, minY: 0, maxY: 1 }, noPadding, 10)[2]).toBe(10);
    expect(camera.fit({ minX: 0, maxX: 1e6, minY: 0, maxY: 1 }, noPadding, 10)[2]).toBe(minZoom);
  });
});

describe('zoomAt', () => {
  it('keeps the point under the pointer in place', () => {
    const camera = createCamera([10, 10, 5], 200, 100);
    const screen: [number, number] = [150, 20];
    const before = camera.toWorld(...screen);

    camera.zoomAt(before, -100);
    expect(camera.position[2]).toBeGreaterThan(5);
    const after = camera.toWorld(...screen);
    expect(after[0]).toBeCloseTo(before[0]);
    expect(after[1]).toBeCloseTo(before[1]);
  });

  it('zooms out for a positive delta and stops at the limits', () => {
    const camera = createCamera([0, 0, 5]);
    camera.zoomAt([0, 0], 100);
    expect(camera.position[2]).toBeLessThan(5);

    camera.zoomAt([0, 0], -1e5);
    expect(camera.position[2]).toBe(maxZoom);
    camera.zoomAt([0, 0], 1e5);
    expect(camera.position[2]).toBe(minZoom);
  });
});

describe('pan', () => {
  it('moves the view against the pointer, scaled by the zoom', () => {
    const camera = createCamera([0, 0, 4]);
    camera.pan([10, 20], 40, -8);
    expect(camera.position).toEqual([0, 22, 4]);
  });
});

// effects only run with the client build of svelte, `vitest --browser=false` uses the server one
describe.skipIf(import.meta.env.SSR)('persistCamera', () => {
  const storage = new Map<string, string>();

  beforeEach(() => {
    storage.clear();
    vi.useFakeTimers();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value)
    });
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('loads the camera of each graph and stores it 500 ms after it moved', () => {
    storage.set('node.cameraPosition.1', '[1,2,3]');
    const graph = $state({ id: 1 });
    const camera = new GraphCamera();
    const stop = persistCamera(camera, () => graph.id);
    flushSync();
    expect(camera.position).toEqual([1, 2, 3]);

    camera.position[0] = 7;
    flushSync();
    vi.advanceTimersByTime(499);
    expect(storage.get('node.cameraPosition.1')).toBe('[1,2,3]');
    vi.advanceTimersByTime(1);
    expect(storage.get('node.cameraPosition.1')).toBe('[7,2,3]');

    graph.id = 2;
    flushSync();
    expect(camera.position).toEqual([140, 100, 3.5]);
    vi.advanceTimersByTime(500);
    expect(storage.get('node.cameraPosition.1')).toBe('[7,2,3]');

    graph.id = 1;
    flushSync();
    expect(camera.position).toEqual([7, 2, 3]);

    stop();
  });

  it('stores nothing once it is stopped', () => {
    const camera = new GraphCamera();
    const stop = persistCamera(camera, () => 1);
    flushSync();
    vi.advanceTimersByTime(500);
    storage.clear();

    stop();
    camera.position[0] = 50;
    flushSync();
    vi.advanceTimersByTime(500);
    expect(storage.size).toBe(0);
  });
});
