import type { GraphCamera } from '../camera.svelte';
import { maxZoom, minZoom } from './constants';

type Point = { x: number; y: number };

/** One finger moves the view, two fingers zoom it. A tap arrives as a mouse click after it. */
export class TouchEventManager {
  private last: { center: Point; distance: number } | undefined;

  constructor(
    private camera: GraphCamera,
    private rect: () => Point
  ) {}

  private measure(touches: TouchList) {
    const [a, b = a] = touches;
    return {
      center: { x: (a.clientX + b.clientX) / 2, y: (a.clientY + b.clientY) / 2 },
      distance: Math.hypot(a.clientX - b.clientX, a.clientY - b.clientY)
    };
  }

  /** Also when a finger is lifted, the rest go on from where they are */
  handleStart(event: TouchEvent) {
    const target = event.target as Element | null;
    const isField = event.touches.length === 1
      && target?.closest?.('input, textarea, select, .add-menu-wrapper');
    this.last = event.touches.length && !isField ? this.measure(event.touches) : undefined;
  }

  handleMove(event: TouchEvent) {
    if (!this.last) return;
    const { center, distance } = this.measure(event.touches);
    const { position, width, height } = this.camera;

    position[0] -= (center.x - this.last.center.x) / position[2];
    position[1] -= (center.y - this.last.center.y) / position[2];

    if (distance > 0 && this.last.distance > 0) {
      const { x, y } = this.rect();
      const zoom = Math.max(
        minZoom,
        Math.min(maxZoom, position[2] * distance / this.last.distance)
      );
      const shift = 1 / position[2] - 1 / zoom;
      position[0] += (center.x - x - width / 2) * shift;
      position[1] += (center.y - y - height / 2) * shift;
      position[2] = zoom;
    }
    this.last = { center, distance };
  }
}
