interface LogEntry {
  time: string;
  scope: string;
  level: string;
  args: unknown[];
}

const logBuffer: LogEntry[] = [];
const startTime = Date.now();

function formatTime(): string {
  const ms = Date.now() - startTime;
  const h = Math.floor(ms / 3600000).toString().padStart(2, '0');
  const m = Math.floor((ms % 3600000) / 60000).toString().padStart(2, '0');
  const s = Math.floor((ms % 60000) / 1000).toString().padStart(2, '0');
  const mss = (ms % 1000).toString().padStart(3, '0');
  return `${h}:${m}:${s}.${mss}`;
}

function serialize(arg: unknown): string {
  if (typeof arg === 'string') return arg;
  try {
    return JSON.stringify(arg);
  } catch {
    return String(arg);
  }
}

function formatEntry(entry: LogEntry, scopeWidth: number): string {
  const scope = `[${entry.scope}]`.padEnd(scopeWidth + 2);
  const level = entry.level.toUpperCase().padEnd(5);
  const msg = entry.args.map(serialize).join(' ');
  return `${entry.time} ${scope} ${level} ${msg}`;
}

(globalThis as Record<string, unknown>).copyLogs = () => {
  if (logBuffer.length === 0) {
    console.log('%c[logger] No log entries to copy', 'color: #888');
    return;
  }
  const scopeWidth = logBuffer.reduce((max, e) => Math.max(max, e.scope.length), 0);
  const lines = [
    `=== Log Export (${logBuffer.length} entries) ===`,
    '',
    ...logBuffer.map(e => formatEntry(e, scopeWidth))
  ].join('\n');

  navigator.clipboard.writeText(lines).then(() => {
    console.log(`%c[logger] Copied ${logBuffer.length} entries to clipboard`, 'color: #4f4');
  });
};

(globalThis as Record<string, unknown>).clearLogs = () => {
  logBuffer.length = 0;
  console.log('%c[logger] Log buffer cleared', 'color: #888');
};

export const createLogger = (() => {
  let maxLength = 5;
  return (scope: string) => {
    maxLength = Math.max(maxLength, scope.length);
    let muted = false;

    let isGrouped = false;

    function s(color: string, ...args: unknown[]) {
      return isGrouped
        ? [...args]
        : [`[%c${scope.padEnd(maxLength, ' ')}]:`, `color: ${color}`, ...args];
    }

    function record(level: string, args: unknown[]) {
      logBuffer.push({ time: formatTime(), scope, level, args });
    }

    return {
      log: (...args: unknown[]) => {
        record('log', args);
        !muted && console.log(...s('#888', ...args));
      },
      info: (...args: unknown[]) => {
        record('info', args);
        !muted && console.info(...s('#888', ...args));
      },
      warn: (...args: unknown[]) => {
        record('warn', args);
        !muted && console.warn(...s('#888', ...args));
      },
      error: (...args: unknown[]) => {
        record('error', args);
        console.error(...s('#f88', ...args));
      },
      group: (...args: unknown[]) => {
        record('group', args);
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
