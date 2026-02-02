export function snapToGrid(value: number, gridSize: number = 10) {
  return Math.round(value / gridSize) * gridSize;
}

export function lerp(a: number, b: number, t: number) {
  return a + (b - a) * t;
}

export function animate(duration: number, callback: (progress: number) => void | false) {
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

export const debounce = (fn: () => void, ms = 300) => {
  let timeoutId: ReturnType<typeof setTimeout>;
  return function(this: unknown, ...args: unknown[]) {
    clearTimeout(timeoutId);
    timeoutId = setTimeout(() => fn.apply(this, args as []), ms);
  };
};

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

export function humanizeNumber(number: number): string {
  const suffixes = ['', 'K', 'M', 'B', 'T'];
  if (number < 1000) {
    return number.toString();
  }
  const numLength = Math.floor(Math.log10(number)) + 1;
  const baseIndex = Math.floor((numLength - 1) / 3);
  const base = Math.pow(10, baseIndex * 3);
  const rounded = Math.round(number / base * 10) / 10;
  return rounded + suffixes[baseIndex];
}

export function humanizeDuration(durationInMilliseconds: number) {
  const millisecondsPerSecond = 1000;
  const millisecondsPerMinute = 60000;
  const millisecondsPerHour = 3600000;
  const millisecondsPerDay = 86400000;

  const days = Math.floor(durationInMilliseconds / millisecondsPerDay);
  const hours = Math.floor((durationInMilliseconds % millisecondsPerDay) / millisecondsPerHour);
  const minutes = Math.floor(
    (durationInMilliseconds % millisecondsPerHour) / millisecondsPerMinute
  );
  const seconds = Math.floor(
    (durationInMilliseconds % millisecondsPerMinute) / millisecondsPerSecond
  );
  const millis = durationInMilliseconds % millisecondsPerSecond;

  let durationString = '';

  if (days > 0) {
    durationString += days + 'd ';
  }
  if (hours > 0) {
    durationString += hours + 'h ';
  }
  if (minutes > 0) {
    durationString += minutes + 'm ';
  }
  if (seconds > 0) {
    durationString += seconds + 's';
  }

  if (millis > 0 || durationString === '') {
    durationString += Math.floor(millis) + 'ms';
  }

  return durationString.trim();
}

export function debounceAsyncFunction<T extends (...args: never[]) => Promise<unknown>>(
  asyncFn: T
): T {
  let isRunning = false;
  let latestArgs: Parameters<T> | null = null;
  let resolveNext: (() => void) | null = null;

  return (async function serializedFunction(...args: Parameters<T>): Promise<ReturnType<T>> {
    latestArgs = args;

    if (isRunning) {
      // Wait for the current execution to finish
      await new Promise<void>((resolve) => {
        resolveNext = resolve;
      });
    }

    // Indicate the function is running
    isRunning = true;

    try {
      // Execute with the latest arguments
      const result = await asyncFn(...latestArgs!);
      return result as ReturnType<T>;
    } finally {
      // Allow the next execution
      isRunning = false;

      if (resolveNext) {
        resolveNext();
        resolveNext = null;
      }
    }
  }) as T;
}

export function withArgsChangeOnly<T extends unknown[], R>(
  func: (...args: T) => R
): (...args: T) => R {
  let lastArgs: T | undefined = undefined;
  let lastResult: R;

  return (...args: T): R => {
    // Check if arguments are the same as last call
    if (
      lastArgs && args.length === lastArgs.length
      && args.every((val, index) => val === lastArgs?.[index])
    ) {
      return lastResult; // Return cached result if arguments haven't changed
    }

    // Call the function with new arguments
    lastResult = func(...args);
    lastArgs = args; // Update stored arguments
    return lastResult; // Return new result
  };
}
