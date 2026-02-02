export function snapToGrid(value: number, gridSize: number = 10) {
  return Math.round(value / gridSize) * gridSize;
}

export function lerp(a: number, b: number, t: number) {
  return a + (b - a) * t;
}

export function animate(
  duration: number,
  callback: (progress: number) => void | false
) {
  const start = performance.now();
  const loop = (time: number) => {
    const progress = (time - start) / duration;
    if (progress < 1) {
      const res = callback(progress);
      if (res !== false) {
        requestAnimationFrame(loop);
      }
    } else {
      callback(1);
    }
  };
  requestAnimationFrame(loop);
}

export function createNodePath({
  depth = 8,
  height = 20,
  y = 50,
  cornerTop = 0,
  cornerBottom = 0,
  leftBump = false,
  rightBump = false,
  aspectRatio = 1
} = {}) {
  return `M0,${cornerTop}
      ${
    cornerTop
      ? ` V${cornerTop}
              Q0,0 ${cornerTop * aspectRatio},0
              H${100 - cornerTop * aspectRatio}
              Q100,0  100,${cornerTop}
            `
      : ` V0
              H100
            `
  }
      V${y - height / 2}
      ${
    rightBump
      ? ` C${100 - depth},${y - height / 2} ${100 - depth},${y + height / 2} 100,${y + height / 2}`
      : ` H100`
  }
      ${
    cornerBottom
      ? ` V${100 - cornerBottom}
              Q100,100 ${100 - cornerBottom * aspectRatio},100
              H${cornerBottom * aspectRatio}
              Q0,100  0,${100 - cornerBottom}
            `
      : `${leftBump ? `V100 H0` : `V100`}`
  }
      ${
    leftBump
      ? ` V${y + height / 2} C${depth},${y + height / 2} ${depth},${y - height / 2} 0,${
        y - height / 2
      }`
      : ` H0`
  }
      Z`.replace(/\s+/g, ' ');
}

export const clone: <T>(v: T) => T = 'structedClone' in globalThis
  ? globalThis.structuredClone
  : (obj) => JSON.parse(JSON.stringify(obj));

export function withSubComponents<A, B extends Record<string, unknown>>(
  component: A,
  subcomponents: B
): A & B {
  Object.keys(subcomponents).forEach((key) => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (component as any)[key] = (subcomponents as any)[key];
  });
  return component as A & B;
}

export function distanceFromPointToSegment(
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  x0: number,
  y0: number
): number {
  const dx = x2 - x1;
  const dy = y2 - y1;

  if (dx === 0 && dy === 0) {
    return Math.hypot(x0 - x1, y0 - y1);
  }

  const t = ((x0 - x1) * dx + (y0 - y1) * dy) / (dx * dx + dy * dy);
  const clampedT = Math.max(0, Math.min(1, t));

  const px = x1 + clampedT * dx;
  const py = y1 + clampedT * dy;

  return Math.hypot(x0 - px, y0 - py);
}
