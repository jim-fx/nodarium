/**
 * Runs the first call right away, then at most once per `delay`. Calls made
 * while waiting are collapsed into one trailing call with the latest arguments,
 * so the final call in a burst is never lost.
 */
export default <T extends unknown[]>(
  callback: (...args: T) => void,
  delay: number
) => {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let pendingArgs: T | undefined;

  const wait = () => {
    timer = setTimeout(() => {
      if (pendingArgs) {
        const args = pendingArgs;
        pendingArgs = undefined;
        callback(...args);
        wait();
      } else {
        timer = undefined;
      }
    }, delay);
  };

  return (...args: T) => {
    if (timer !== undefined) {
      pendingArgs = args;
      return;
    }
    callback(...args);
    wait();
  };
};
