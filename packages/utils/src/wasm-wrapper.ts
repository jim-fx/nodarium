import type { NodeValue, WasmSlice } from '@nodarium/types';
import { concatEncodedArrays } from './flatTree';

// Nodarium ABI v1, see docs/ABI.md
interface NodariumExports extends WebAssembly.Exports {
  memory: WebAssembly.Memory;
  nodarium_alloc: (bytes: number) => number;
  nodarium_reset: () => void;
  nodarium_execute: (argsPtr: number, argc: number) => number;
}

// Pre-v1 nodes, which may still be served from the registry cache
interface LegacyExports extends WebAssembly.Exports {
  memory: WebAssembly.Memory;
  execute: (ptr: number, len: number) => number;
  __free: (ptr: number, len: number) => void;
  __alloc: (len: number) => number;
}

export function isWasmSlice(value: NodeValue): value is WasmSlice {
  return !(value instanceof Int32Array);
}

/** Returns a copy of the value that stays valid after its module is reset */
export function readNodeValue(value: NodeValue): Int32Array {
  if (!isWasmSlice(value)) return value;
  return new Int32Array(value.memory.buffer, value.ptr, value.len).slice();
}

function viewNodeValue(value: NodeValue): Int32Array {
  if (!isWasmSlice(value)) return value;
  return new Int32Array(value.memory.buffer, value.ptr, value.len);
}

export function createWasmWrapper(buffer: ArrayBuffer) {
  let memory: WebAssembly.Memory | undefined;

  const logPanic = (ptr: number, len: number) => {
    if (!memory) return;
    const view = new Uint8Array(memory.buffer, ptr, len);
    console.error('RUST PANIC:', new TextDecoder().decode(view));
  };
  const log = (ptr: number, len: number) => {
    if (!memory) return;
    const view = new Uint8Array(memory.buffer, ptr, len);
    console.log('RUST:', new TextDecoder().decode(view));
  };

  const importObject = {
    env: {
      nodarium_panic: logPanic,
      nodarium_log: log,
      host_log_panic: logPanic,
      host_log: log
    }
  };

  const module = new WebAssembly.Module(buffer);
  const instance = new WebAssembly.Instance(module, importObject);
  memory = instance.exports.memory as WebAssembly.Memory;
  const mem = memory;

  function executeV1(exports: NodariumExports, inputs: NodeValue[]): WasmSlice {
    const table = new Uint32Array(inputs.length * 2);

    for (let i = 0; i < inputs.length; i++) {
      const input = inputs[i];

      // Results of this module are already in our memory
      if (isWasmSlice(input) && input.memory === mem) {
        table[i * 2] = input.ptr;
        table[i * 2 + 1] = input.len;
        continue;
      }

      const len = input instanceof Int32Array ? input.length : input.len;
      const ptr = exports.nodarium_alloc(len * 4);
      // create views after alloc, as it may grow (and detach) the memory
      new Int32Array(mem.buffer, ptr, len).set(viewNodeValue(input));
      table[i * 2] = ptr;
      table[i * 2 + 1] = len;
    }

    const tablePtr = exports.nodarium_alloc(table.byteLength);
    new Uint32Array(mem.buffer, tablePtr, table.length).set(table);

    const resultPtr = exports.nodarium_execute(tablePtr, inputs.length);
    const [ptr, len] = new Uint32Array(mem.buffer, resultPtr, 2);

    return { memory: mem, ptr, len };
  }

  function executeLegacy(exports: LegacyExports, inputs: NodeValue[]): Int32Array {
    // the legacy ABI expects all inputs concatenated, with scalars inlined
    const args = concatEncodedArrays(
      inputs.map(input => {
        const value = readNodeValue(input);
        return value.length === 1 ? value[0] : value;
      })
    );

    const inPtr = exports.__alloc(args.length);
    new Int32Array(mem.buffer).set(args, inPtr / 4);

    const outPtr = exports.execute(inPtr, args.length);

    const i32Result = new Int32Array(mem.buffer);
    const outLen = i32Result[outPtr / 4];
    const out = i32Result.slice(outPtr / 4 + 1, outPtr / 4 + 1 + outLen);

    exports.__free(inPtr, args.length);

    return out;
  }

  const isV1 = typeof instance.exports.nodarium_execute === 'function';

  function execute(inputs: NodeValue[]): NodeValue {
    if (isV1) return executeV1(instance.exports as NodariumExports, inputs);
    return executeLegacy(instance.exports as LegacyExports, inputs);
  }

  function reset() {
    if (isV1) (instance.exports as NodariumExports).nodarium_reset();
  }

  function get_definition() {
    const sections = WebAssembly.Module.customSections(module, 'nodarium_definition');
    if (sections.length > 0) {
      const decoder = new TextDecoder();
      const jsonString = decoder.decode(sections[0]);
      return JSON.parse(jsonString);
    }
  }

  return { execute, reset, get_definition };
}
