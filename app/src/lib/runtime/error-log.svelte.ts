import type { RuntimeErrorInfo } from './errors';

export type ErrorLogEntry = RuntimeErrorInfo & { id: number; time: number };

const MAX_ENTRIES = 100;

let entries = $state<ErrorLogEntry[]>([]);
let nextId = 0;

export const errorLog = {
  /** Newest entry first */
  get entries() {
    return entries;
  },
  add(info: RuntimeErrorInfo) {
    entries = [{ ...info, id: nextId++, time: Date.now() }, ...entries].slice(0, MAX_ENTRIES);
  },
  clear() {
    entries = [];
  }
};
