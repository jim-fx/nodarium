export function logInt32ArrayChanges(
  before: Int32Array,
  after: Int32Array,
  clamp = 10
): void {
  if (before.length !== after.length) {
    throw new Error('Arrays must have the same length');
  }

  let rangeStart: number | null = null;
  let collected: number[] = [];

  const flush = (endIndex: number) => {
    if (rangeStart === null) return;

    const preview = collected.slice(0, clamp);
    const suffix = collected.length > clamp ? '...' : '';

    console.log(
      `Change ${rangeStart}-${endIndex}: [${preview.join(', ')}${suffix}]`
    );

    rangeStart = null;
    collected = [];
  };

  for (let i = 0; i < before.length; i++) {
    if (before[i] !== after[i]) {
      if (rangeStart === null) {
        rangeStart = i;
      }
      collected.push(after[i]);
    } else {
      flush(i - 1);
    }
  }

  flush(before.length - 1);
}
