interface LogEntry {
  time: string;
  scope: string;
  level: string;
  args: unknown[];
}

export const LOG_BUFFER_SIZE = 2000;

// Entries only hold summaries, a logged value may be a view into a wasm memory, which would
// keep that whole memory alive
const logBuffer: LogEntry[] = [];
let logStart = 0;
const startTime = Date.now();

const MAX_ITEMS = 8;
const MAX_DEPTH = 2;
const MAX_STRING = 500;

function formatTime(): string {
  const ms = Date.now() - startTime;
  const h = Math.floor(ms / 3600000).toString().padStart(2, '0');
  const m = Math.floor((ms % 3600000) / 60000).toString().padStart(2, '0');
  const s = Math.floor((ms % 60000) / 1000).toString().padStart(2, '0');
  const mss = (ms % 1000).toString().padStart(3, '0');
  return `${h}:${m}:${s}.${mss}`;
}

function typeName(value: object): string {
  return value.constructor?.name ?? 'Object';
}

function summarizeItems(items: ArrayLike<unknown>, depth: number): string {
  const n = Math.min(items.length, MAX_ITEMS);
  const parts: string[] = [];
  for (let i = 0; i < n; i++) parts.push(summarize(items[i], depth));
  if (items.length > n) parts.push('…');
  return parts.join(', ');
}

/** A short string for any value, nested values only to MAX_DEPTH and MAX_ITEMS */
function summarize(value: unknown, depth = MAX_DEPTH): string {
  switch (typeof value) {
    case 'string':
      return value.length > MAX_STRING ? value.slice(0, MAX_STRING) + '…' : value;
    case 'number':
    case 'boolean':
    case 'bigint':
    case 'undefined':
    case 'symbol':
      return String(value);
    case 'function':
      return `[Function ${value.name || 'anonymous'}]`;
  }
  if (value === null) return 'null';
  const obj = value as object;
  if (obj instanceof Error) return `${obj.name}: ${obj.message}`;
  if (obj instanceof ArrayBuffer) return `ArrayBuffer(${obj.byteLength})`;
  if (obj instanceof DataView) return `DataView(${obj.byteLength})`;
  if (ArrayBuffer.isView(obj)) {
    const items = obj as unknown as ArrayLike<number>;
    const head = depth > 0 ? ' [' + summarizeItems(items, 0) + ']' : '';
    return `${typeName(obj)}(${items.length})${head}`;
  }
  if (Array.isArray(obj)) {
    return depth > 0 ? '[' + summarizeItems(obj, depth - 1) + ']' : `Array(${obj.length})`;
  }
  if (obj instanceof Map || obj instanceof Set) return `${typeName(obj)}(${obj.size})`;
  if (depth <= 0) return typeName(obj);
  const keys = Object.keys(obj);
  const parts = keys.slice(0, MAX_ITEMS).map(k =>
    `${k}: ${summarize((obj as Record<string, unknown>)[k], depth - 1)}`
  );
  if (keys.length > MAX_ITEMS) parts.push('…');
  return '{' + parts.join(', ') + '}';
}

/** Like summarize, but a value whose getters or proxy traps throw only gets its type */
function summarizeSafely(value: unknown): string {
  try {
    return summarize(value);
  } catch {
    return `[unloggable ${typeof value}]`;
  }
}

function record(entry: LogEntry) {
  if (logBuffer.length < LOG_BUFFER_SIZE) {
    logBuffer.push(entry);
  } else {
    logBuffer[logStart] = entry;
    logStart = (logStart + 1) % LOG_BUFFER_SIZE;
  }
}

/** The recorded entries, oldest first */
export function getLogEntries(): LogEntry[] {
  return [...logBuffer.slice(logStart), ...logBuffer.slice(0, logStart)];
}

export function clearLogs() {
  logBuffer.length = 0;
  logStart = 0;
}

function formatEntry(entry: LogEntry, scopeWidth: number): string {
  const scope = `[${entry.scope}]`.padEnd(scopeWidth + 2);
  const level = entry.level.toUpperCase().padEnd(5);
  const msg = entry.args.join(' ');
  return `${entry.time} ${scope} ${level} ${msg}`;
}

(globalThis as Record<string, unknown>).copyLogs = () => {
  const entries = getLogEntries();
  if (entries.length === 0) {
    console.log('%c[logger] No log entries to copy', 'color: #888');
    return;
  }
  const scopeWidth = entries.reduce((max, e) => Math.max(max, e.scope.length), 0);
  const lines = [
    `=== Log Export (${entries.length} entries) ===`,
    '',
    ...entries.map(e => formatEntry(e, scopeWidth))
  ].join('\n');

  navigator.clipboard.writeText(lines).then(() => {
    console.log(`%c[logger] Copied ${entries.length} entries to clipboard`, 'color: #4f4');
  });
};

(globalThis as Record<string, unknown>).clearLogs = () => {
  clearLogs();
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

    // muted scopes such as the runtime log every node, they would flush the buffer on every run
    function add(level: string, args: unknown[]) {
      if (muted && level !== 'warn' && level !== 'error') return;
      record({ time: formatTime(), scope, level, args: args.map(summarizeSafely) });
    }

    return {
      log: (...args: unknown[]) => {
        add('log', args);
        !muted && console.log(...s('#888', ...args));
      },
      info: (...args: unknown[]) => {
        add('info', args);
        !muted && console.info(...s('#888', ...args));
      },
      warn: (...args: unknown[]) => {
        add('warn', args);
        !muted && console.warn(...s('#888', ...args));
      },
      error: (...args: unknown[]) => {
        add('error', args);
        console.error(...s('#f88', ...args));
      },
      group: (...args: unknown[]) => {
        add('group', args);
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
