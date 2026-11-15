import { getErrorInfo, type RuntimeErrorInfo } from '@nodarium/runtime';
import type { Execution, WorkerExecution } from './types';

/** A run that failed in the worker, with what the worker knew about it */
export class WorkerRunError extends Error {
  constructor(public readonly info: RuntimeErrorInfo) {
    super(info.message);
    this.name = 'WorkerRunError';
  }
}

/** The execution of an answer of the worker, a failed run throws as it does on the main thread */
export function unwrapExecution(answer: WorkerExecution): Execution {
  if (!answer.ok) throw new WorkerRunError(answer.error);
  return answer;
}

/** What is known about a failed run, wherever it ran */
export function errorInfo(error: unknown): RuntimeErrorInfo {
  return error instanceof WorkerRunError ? error.info : getErrorInfo(error);
}
