import type { Box } from '@nodarium/runtime';
import { untrack } from 'svelte';
import { maxZoom, minZoom, zoomSpeed } from './graph/constants';
import { animate, debounce, easeInOut, lerp } from './helpers/index.ts';

/** x, y and zoom */
export type CameraPosition = [number, number, number];

export type Padding = { left: number; right: number; top: number; bottom: number };

const defaultPosition = (): CameraPosition => [140, 100, 3.5];

export class GraphCamera {
  position: CameraPosition = $state(defaultPosition());
  /** the position is the one the user left the graph at */
  restored = false;
  /** the size of the view in px */
  width = $state(100);
  height = $state(100);

  /** the part of the graph that is in the view */
  bounds: Box = $derived({
    minX: this.position[0] - this.width / this.position[2] / 2,
    maxX: this.position[0] + this.width / this.position[2] / 2,
    minY: this.position[1] - this.height / this.position[2] / 2,
    maxY: this.position[1] + this.height / this.position[2] / 2
  });

  /** A point of the view in px to graph units */
  toWorld(x: number, y: number): [number, number] {
    return [
      this.position[0] + (x - this.width / 2) / this.position[2],
      this.position[1] + (y - this.height / 2) / this.position[2]
    ];
  }

  /** How many grid steps per 5 graph units a node snaps to: 2 once the grid of 2.5 units is drawn, 1 before */
  snapLevel() {
    const z = this.position[2];
    if (z > 11) return 2;
    return 1;
  }

  /** The position that shows `box` in the view without `padding` */
  fit(box: Box, padding: Padding, maxFitZoom: number): CameraPosition {
    const availableWidth = Math.max(this.width - padding.left - padding.right, 1);
    const availableHeight = Math.max(this.height - padding.top - padding.bottom, 1);

    const zoom = Math.max(
      minZoom,
      Math.min(
        maxFitZoom,
        availableWidth / (box.maxX - box.minX),
        availableHeight / (box.maxY - box.minY)
      )
    );

    // the centre of the box lands in the centre of the area that is not padded
    return [
      (box.minX + box.maxX) / 2 - (padding.left - padding.right) / 2 / zoom,
      (box.minY + box.maxY) / 2 - (padding.top - padding.bottom) / 2 / zoom,
      zoom
    ];
  }

  /** Glides to `target`, ends early once `stop` returns true */
  animateTo(target: CameraPosition, stop: () => boolean) {
    const from = [...this.position];
    // position and zoom share one easing, so the view stays consistent while it moves
    animate(500, (a: number) => {
      const t = easeInOut(a);
      this.position[0] = lerp(from[0], target[0], t);
      this.position[1] = lerp(from[1], target[1], t);
      this.position[2] = lerp(from[2], target[2], t);
      if (stop()) return false;
    });
  }

  /** Zooms by a wheel delta so that `point` (graph units) stays under the pointer */
  zoomAt(point: [number, number], deltaY: number) {
    const delta = Math.pow(0.95, zoomSpeed * Math.abs(deltaY * 0.01));
    const zoom = this.position[2];
    const newZoom = Math.max(
      minZoom,
      Math.min(maxZoom, deltaY < 0 ? zoom / delta : zoom * delta)
    );
    const zoomRatio = newZoom / zoom;

    this.position[0] = point[0] - (point[0] - this.position[0]) / zoomRatio;
    this.position[1] = point[1] - (point[1] - this.position[1]) / zoomRatio;
    this.position[2] = newZoom;
  }

  /** Moves the view from where it was at `from` by the pointer movement `dx`, `dy` in px */
  pan(from: [number, number], dx: number, dy: number) {
    this.position[0] = from[0] - dx / this.position[2];
    this.position[1] = from[1] - dy / this.position[2];
  }
}

const storageKey = (id: number) => `node.cameraPosition.${id}`;

/** Loads the camera of the graph and stores it when it moved. Returns a function that stops both. */
export function persistCamera(camera: GraphCamera, graphId: () => number): () => void {
  // undefined while the camera of the next graph loads, so it is not stored under the old id
  let loadedId: number | undefined;

  const save = debounce(() => {
    if (loadedId === undefined) return;
    localStorage.setItem(storageKey(loadedId), JSON.stringify(camera.position));
  }, 500);

  return $effect.root(() => {
    $effect(() => {
      const id = graphId();
      untrack(() => {
        loadedId = undefined;
        const stored = localStorage.getItem(storageKey(id));
        camera.position = defaultPosition();
        camera.restored = stored !== null;
        if (stored) {
          try {
            const d = JSON.parse(stored);
            camera.position[0] = d[0];
            camera.position[1] = d[1];
            camera.position[2] = d[2];
          } catch (e) {
            console.log('Failed to parsed stored camera position', e);
          }
        }
        loadedId = id;
      });
    });

    $effect(() => {
      void camera.position[0];
      void camera.position[1];
      void camera.position[2];
      save();
    });
  });
}
