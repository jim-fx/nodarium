import type { RunResult, RuntimeErrorInfo } from '@nodarium/runtime';
import type { Graph } from '@nodarium/types';

/** What the app reads of a run, a run in the worker brings the timings of the worker along */
export type Execution = RunResult & { performance?: Record<string, number[]> };

/** What the worker answers, an error thrown through Comlink would only keep its message */
export type WorkerExecution =
  | ({ ok: true } & Execution)
  | { ok: false; error: RuntimeErrorInfo };

/** A runtime of the app, on the main thread or in the worker */
export interface AppRuntime {
  run(graph: Graph, settings: Record<string, unknown>): Promise<Execution>;
}
