/**
 * Resolves once `condition` is true. It is read again whenever the reactive state it reads
 * changes, so everything that decides it has to be `$state`.
 */
export function when(condition: () => boolean): Promise<void> {
  return new Promise((resolve) => {
    const stop = $effect.root(() => {
      $effect(() => {
        if (!condition()) return;
        resolve();
        queueMicrotask(stop);
      });
    });
  });
}
