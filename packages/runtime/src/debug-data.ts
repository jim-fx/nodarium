import type { NodeValue } from '@nodarium/types';
import { getOutputType } from '@nodarium/types';
import { decodeFloat, readNodeValue } from '@nodarium/utils';
import { patternPreview } from './pattern-preview';
import type { PlanNode } from './plan';
import type { DebugEntry } from './types';

/**
 * Turns a value of this socket type into geometry the debug view can draw, undefined when the
 * words aren't one. The output shows only geometry, these are views for the debug node.
 */
const DEBUG_PREVIEWS: Record<string, (words: Int32Array) => Int32Array | undefined> = {
  // a flat disc colored by the pattern
  pattern: patternPreview
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

  /** Values the viewer can't draw as they are get turned into geometry it can draw */
  preview() {
    for (const entry of Object.values(this.data)) {
      const preview = DEBUG_PREVIEWS[entry.type]?.(entry.data);
      if (preview) entry.data = preview;
    }
  }
}
