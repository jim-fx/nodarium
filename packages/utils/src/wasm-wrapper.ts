interface NodariumExports extends WebAssembly.Exports {
  memory: WebAssembly.Memory;
  execute: (ptr: number, len: number) => number;
  __free: (ptr: number, len: number) => void;
  __alloc: (len: number) => number;
}

export function createWasmWrapper(buffer: ArrayBuffer) {
  let exports: NodariumExports;

  const importObject = {
    env: {
      host_log_panic: (ptr: number, len: number) => {
        if (!exports) return;
        const view = new Uint8Array(exports.memory.buffer, ptr, len);
        console.error('RUST PANIC:', new TextDecoder().decode(view));
      },
      host_log: (ptr: number, len: number) => {
        if (!exports) return;
        const view = new Uint8Array(exports.memory.buffer, ptr, len);
        console.log('RUST:', new TextDecoder().decode(view));
      }
    }
  };

  const module = new WebAssembly.Module(buffer);
  const instance = new WebAssembly.Instance(module, importObject);
  exports = instance.exports as NodariumExports;

  function execute(args: Int32Array) {
    const inPtr = exports.__alloc(args.length);
    new Int32Array(exports.memory.buffer).set(args, inPtr / 4);

    const outPtr = exports.execute(inPtr, args.length);

    const i32Result = new Int32Array(exports.memory.buffer);
    const outLen = i32Result[outPtr / 4];
    const out = i32Result.slice(outPtr / 4 + 1, outPtr / 4 + 1 + outLen);

    exports.__free(inPtr, args.length);

    return out;
  }

  function get_definition() {
    const sections = WebAssembly.Module.customSections(module, 'nodarium_definition');
    if (sections.length > 0) {
      const decoder = new TextDecoder();
      const jsonString = decoder.decode(sections[0]);
      return JSON.parse(jsonString);
    }
  }

  return { execute, get_definition };
}
