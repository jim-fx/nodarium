export const createLogger = (() => {
  let maxLength = 5;
  return (scope: string) => {
    maxLength = Math.max(maxLength, scope.length);
    let muted = false;

    let isGrouped = false;

    function s(color: string, ...args: any) {
      return isGrouped
        ? [...args]
        : [`[%c${scope.padEnd(maxLength, ' ')}]:`, `color: ${color}`, ...args];
    }

    return {
      log: (...args: any[]) => !muted && console.log(...s('#888', ...args)),
      info: (...args: any[]) => !muted && console.info(...s('#888', ...args)),
      warn: (...args: any[]) => !muted && console.warn(...s('#888', ...args)),
      error: (...args: any[]) => console.error(...s('#f88', ...args)),
      group: (...args: any[]) => {
        if (!muted) {
          console.groupCollapsed(...s('#888', ...args));
          isGrouped = true;
        }
      },
      groupEnd: () => {
        if (!muted) {
          console.groupEnd();
          isGrouped = false;
        }
      },
      mute() {
        muted = true;
      },
      unmute() {
        muted = false;
      }
    };
  };
})();
