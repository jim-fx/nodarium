import { FileNodeRegistry } from '@nodarium/runtime/node';
import type { NodeDefinition, NodeValue } from '@nodarium/types';
import { isWasmSlice, readNodeValue } from '@nodarium/utils';

/** What one execute of a node took in and holds, the runtime measures its time and output */
export type NodeCall = {
  type: string;
  /** bytes of inputs that came from JS, copied into the node's memory */
  inJs: number;
  /** bytes of inputs that came from the memory of another node, copied */
  inWasm: number;
  /** bytes of inputs that were in the node's own memory already, not copied */
  inShared: number;
  /** of the node's wasm memory after the call */
  memory: number;
};

/**
 * The nodes of a folder, each execute recorded in {@link calls}. Nothing of the runtime changes:
 * the bench uses a plain FileNodeRegistry, the profile this one.
 */
export class ProfilingRegistry extends FileNodeRegistry {
  calls: NodeCall[] = [];
  resetMs = 0;
  private wrapped = new WeakMap<NodeDefinition, NodeDefinition>();

  /** @param dump gets copies of the inputs of every call, before the node runs */
  constructor(dir: string, private dump?: (type: string, inputs: Int32Array[]) => void) {
    super(dir);
  }

  /** Empties {@link calls} and {@link resetMs} for the next build */
  startBuild() {
    this.calls = [];
    this.resetMs = 0;
  }

  override getNode(id: string) {
    const definition = super.getNode(id);
    return definition && this.wrap(definition);
  }

  override getAllNodes() {
    return super.getAllNodes().map(definition => this.wrap(definition));
  }

  private wrap(definition: NodeDefinition) {
    let wrapped = this.wrapped.get(definition);
    if (wrapped) return wrapped;
    wrapped = {
      ...definition,
      execute: inputs => this.execute(definition, inputs),
      reset: definition.reset && (() => {
        const a = performance.now();
        definition.reset!();
        this.resetMs += performance.now() - a;
      })
    };
    this.wrapped.set(definition, wrapped);
    return wrapped;
  }

  private execute(definition: NodeDefinition, inputs: NodeValue[]) {
    this.dump?.(definition.id, inputs.map(readNodeValue));
    const result = definition.execute(inputs);
    const outputs = Array.isArray(result) ? result : [result];
    const own = outputs.find(isWasmSlice)?.memory;
    const call: NodeCall = {
      type: definition.id,
      inJs: 0,
      inWasm: 0,
      inShared: 0,
      memory: definition.memoryBytes?.() ?? 0
    };
    for (const input of inputs) {
      if (!isWasmSlice(input)) call.inJs += input.byteLength;
      else if (input.memory === own) call.inShared += input.len * 4;
      else call.inWasm += input.len * 4;
    }
    this.calls.push(call);
    return result;
  }
}
