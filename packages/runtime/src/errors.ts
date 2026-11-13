import type { NodeDefinition } from '@nodarium/types';
import { WasmPanicError } from '@nodarium/utils';

/** Plain description of a failed run, survives the transfer out of the worker */
export type RuntimeErrorInfo = {
  message: string;
  /** Node that failed, for nodes inside a group this is the group node in the editor */
  nodeId?: number;
  nodeType?: string;
  nodeTitle?: string;
  /** Inputs of the failed node that are neither connected nor have a value */
  missingInputs?: string[];
  /** Source location of a Rust panic */
  location?: string;
};

export class NodeExecutionError extends Error {
  info: RuntimeErrorInfo;

  constructor(
    cause: unknown,
    node: { id: number; type: string },
    definition?: NodeDefinition,
    missingInputs: string[] = []
  ) {
    const reason = cause instanceof Error ? cause.message : String(cause);
    const title = definition?.meta?.title || node.type.split('/').pop() || node.type;
    let message = `${title} failed: ${reason}`;
    if (missingInputs.length) {
      const names = missingInputs.map(i => `"${i}"`).join(', ');
      message += ` (unconnected ${missingInputs.length > 1 ? 'inputs' : 'input'} ${names})`;
    }
    super(message, { cause });
    this.name = 'NodeExecutionError';
    this.info = {
      message,
      nodeId: node.id,
      nodeType: node.type,
      nodeTitle: title,
      missingInputs: missingInputs.length ? missingInputs : undefined,
      location: cause instanceof WasmPanicError ? cause.location : undefined
    };
  }
}

export function getErrorInfo(error: unknown): RuntimeErrorInfo {
  if (error instanceof NodeExecutionError) return error.info;
  return { message: error instanceof Error ? error.message : String(error) };
}
