interface NodariumExports extends WebAssembly.Exports {
  memory: WebAssembly.Memory;
  execute: (outputPos: number, ...args: number[]) => number;
}

export function createWasmWrapper(buffer: ArrayBuffer, memory: WebAssembly.Memory) {
  let exports: NodariumExports;

  const importObject = {
    env: {
      memory: memory,
      __nodarium_log_panic: (ptr: number, len: number) => {
        if (!exports) return;
        const view = new Uint8Array(exports.memory.buffer, ptr, len);
        console.error('RUST PANIC:', new TextDecoder().decode(view));
      },
      __nodarium_log: (ptr: number, len: number) => {
        if (!exports) return;
        const view = new Uint8Array(exports.memory.buffer, ptr, len);
        console.log('RUST:', new TextDecoder().decode(view));
      }
    }
  };

  const module = new WebAssembly.Module(buffer);
  const instance = new WebAssembly.Instance(module, importObject);
  exports = instance.exports as NodariumExports;

  function execute(outputPos: number, args: number[]): number {
    return exports.execute(outputPos, ...args);
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
