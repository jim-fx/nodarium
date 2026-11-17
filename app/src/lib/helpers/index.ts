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

/**
 * Runs `asyncFn` once at a time. The calls that come while it runs are served by one run with
 * the arguments of the last of them, and settle with that run.
 */
export function debounceAsyncFunction<T extends (...args: never[]) => Promise<unknown>>(
  asyncFn: T
): T {
  type Result = Awaited<ReturnType<T>>;
  let running = false;
  let queued:
    | { args: Parameters<T>; result: Promise<Result>; settle: (run: Promise<Result>) => void }
    | undefined;

  function start(args: Parameters<T>): Promise<Result> {
    running = true;
    const run = asyncFn(...args) as Promise<Result>;
    run.catch(() => {}).then(() => {
      const next = queued;
      queued = undefined;
      // started in the same microtask, a call in between would run next to it
      if (next) next.settle(start(next.args));
      else running = false;
    });
    return run;
  }

  return ((...args: Parameters<T>) => {
    if (!running) return start(args);
    if (queued) {
      queued.args = args;
      return queued.result;
    }
    let settle!: (run: Promise<Result>) => void;
    const result = new Promise<Result>((resolve) => (settle = resolve));
    queued = { args, result, settle };
    return result;
  }) as T;
}
