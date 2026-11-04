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
      await new Promise<void>((resolve) => {
        resolveNext = resolve;
      });
    }

    isRunning = true;

    try {
      const result = await asyncFn(...latestArgs!);
      return result as ReturnType<T>;
    } finally {
      isRunning = false;

      if (resolveNext) {
        resolveNext();
        resolveNext = null;
      }
    }
  }) as T;
}
