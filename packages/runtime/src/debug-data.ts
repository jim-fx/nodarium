import type { NodeDefinition, NodeValue } from '@nodarium/types';
import { getOutputType } from '@nodarium/types';
import { createLogger, decodeFloat, readNodeValue } from '@nodarium/utils';
import type { PlanNode } from './plan';
import type { DebugEntry } from './types';

const log = createLogger('runtime-executor');
log.mute();

/** The node that turns a value of this socket type into something the debug view can draw */
const DEBUG_PREVIEWS: Record<string, { node: string }> = {
  // a flat disc colored by the pattern
  pattern: { node: 'max/myzel/output' }
};

/** What the debug nodes of one run show */
export class DebugCollector {
  /** keyed by the id of the node a debug node is connected to */
  readonly data: Record<number, DebugEntry> = {};

  /** Copies the value of the output a debug node shows, for every other node it does nothing */
  record(node: PlanNode, values: NodeValue[]) {
    const output = node.debugOutput;
    const outputs = node.definition.outputs;
    if (output === undefined || !outputs) return;
    const value = values[output];
    if (value === undefined) return;
    this.data[node.id] = {
      type: getOutputType(outputs[output]) ?? 'unknown',
      output,
      data: readNodeValue(value)
    };
  }

  /**
   * The amount and clip of a debug node go along with the data it shows, the viewer applies
   * them. So the output of the debugged node and every cache key stay as they are.
   */
  applyView(debugNode: PlanNode, inputs: NodeValue[]) {
    const keys = Object.keys(debugNode.definition.inputs ?? {});
    const shown = debugNode.inputs[keys.indexOf('input')];
    const entry = shown && !(shown instanceof Int32Array) ? this.data[shown.node.id] : undefined;
    if (!entry) return;
    for (const key of ['amount', 'clip'] as const) {
      const value = inputs[keys.indexOf(key)];
      if (value !== undefined) entry[key] = decodeFloat(readNodeValue(value)[0]);
    }
  }

  /**
   * Values the viewer can't draw as they are get turned into something it can, by a node of the
   * graph's own collection. A pattern is only numbers, its preview is the mesh of the output node.
   */
  preview(definitions: Map<string, NodeDefinition>) {
    for (const entry of Object.values(this.data)) {
      const preview = DEBUG_PREVIEWS[entry.type];
      const node = preview && definitions.get(preview.node);
      if (!node) continue;
      try {
        const result = node.execute([entry.data]);
        entry.data = readNodeValue(Array.isArray(result) ? result[0] : result);
      } catch (e) {
        log.warn(`could not preview the ${entry.type} for debugging`, e);
      }
    }
  }
}
