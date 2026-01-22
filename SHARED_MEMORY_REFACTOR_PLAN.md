# Shared Memory Refactor Plan

## Executive Summary

Migrate to a single shared `WebAssembly.Memory` instance imported by all nodes using `--import-memory`. The `#[nodarium_execute]` macro writes the function's return value directly to shared memory at the specified offset.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Shared WebAssembly.Memory                    │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │  [Node A output]  [Node B output]  [Node C output]  ...       │  │
│  │  ┌────────────┐   ┌────────────┐   ┌────────────┐             │  │
│  │  │ Vec<i32>   │   │ Vec<i32>   │   │ Vec<i32>   │             │  │
│  │  │ 4 bytes    │   │ 12 bytes   │   │ 2KB        │             │  │
│  │  └────────────┘   └────────────┘   └────────────┘             │  │
│  │                                                               │  │
│  │  offset: 0 ────────────────────────────────────────────────►  │  │
│  └───────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                                   ▲
                                   │
                                   │ import { memory } from "env"
         ┌─────────────────────────┼─────────────────────────┐
         │                         │                         │
    ┌────┴────┐               ┌────┴────┐               ┌────┴────┐
    │  Node A │               │  Node B │               │  Node C │
    │  WASM   │               │  WASM   │               │  WASM   │
    └─────────┘               └─────────┘               └─────────┘
```

## Phase 1: Compilation Configuration

### 1.1 Cargo Config

```toml
# nodes/max/plantarium/box/.cargo/config.toml
[build]
rustflags = ["-C", "link-arg=--import-memory"]
```

Or globally in `Cargo.toml`:

```toml
[profile.release]
rustflags = ["-C", "link-arg=--import-memory"]
```

### 1.2 Import Memory Semantics

With `--import-memory`:

- Nodes **import** memory from the host (not export their own)
- All nodes receive the same `WebAssembly.Memory` instance
- Memory is read/write accessible from all modules
- No `memory.grow` needed (host manages allocation)

## Phase 2: Macro Design

### 2.1 Clean Node API

```rust
// input.json has 3 inputs: op_type, a, b
nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(op_type: *i32, a: *i32, b: *i32) -> Vec<i32> {
    // Read inputs directly from shared memory
    let op = unsafe { *op_type };
    let a_val = f32::from_bits(unsafe { *a } as u32);
    let b_val = f32::from_bits(unsafe { *b } as u32);

    let result = match op {
        0 => a_val + b_val,
        1 => a_val - b_val,
        2 => a_val * b_val,
        3 => a_val / b_val,
        _ => 0.0,
    };

    // Return Vec<i32>, macro handles writing to shared memory
    vec![result.to_bits()]
}
```

### 2.2 Macro Implementation

```rust
// packages/macros/src/lib.rs

#[proc_macro_attribute]
pub fn nodarium_execute(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as syn::ItemFn);
    let fn_name = &input_fn.sig.ident;

    // Parse definition to get input count
    let project_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let def: NodeDefinition = serde_json::from_str(&fs::read_to_string(
        Path::new(&project_dir).join("src/input.json")
    ).unwrap()).unwrap();

    let input_count = def.inputs.as_ref().map(|i| i.len()).unwrap_or(0);

    // Validate signature
    validate_signature(&input_fn, input_count);

    // Generate wrapper
    generate_execute_wrapper(input_fn, fn_name, input_count)
}

fn validate_signature(fn_sig: &syn::Signature, expected_inputs: usize) {
    let param_count = fn_sig.inputs.len();
    if param_count != expected_inputs {
        panic!(
            "Execute function has {} parameters but definition has {} inputs\n\
             Definition inputs: {:?}\n\
             Expected signature:\n\
             pub fn execute({}) -> Vec<i32>",
            param_count,
            expected_inputs,
            def.inputs.as_ref().map(|i| i.keys().collect::<Vec<_>>()),
            (0..expected_inputs)
                .map(|i| format!("arg{}: *const i32", i))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    // Verify return type is Vec<i32>
    match &fn_sig.output {
        syn::ReturnType::Type(_, ty) => {
            if !matches!(&**ty, syn::Type::Path(tp) if tp.path.is_ident("Vec")) {
                panic!("Execute function must return Vec<i32>");
            }
        }
        syn::ReturnType::Default => {
            panic!("Execute function must return Vec<i32>");
        }
    }
}

fn generate_execute_wrapper(
    input_fn: syn::ItemFn,
    fn_name: &syn::Ident,
    input_count: usize,
) -> TokenStream {
    let arg_names: Vec<_> = (0..input_count)
        .map(|i| syn::Ident::new(&format!("arg{}", i), proc_macro2::Span::call_site()))
        .collect();

    let expanded = quote! {
        #input_fn

        #[no_mangle]
        pub extern "C" fn execute(
            output_pos: i32,
            #( #arg_names: i32 ),*
        ) -> i32 {
            extern "C" {
                fn __nodarium_log(ptr: *const u8, len: usize);
                fn __nodarium_log_panic(ptr: *const u8, len: usize);
            }

            // Setup panic hook
            static SET_HOOK: std::sync::Once = std::sync::Once::new();
            SET_HOOK.call_once(|| {
                std::panic::set_hook(Box::new(|info| {
                    let msg = info.to_string();
                    unsafe { __nodarium_log_panic(msg.as_ptr(), msg.len()); }
                }));
            });

            // Call user function
            let result = #fn_name(
                #( #arg_names as *const i32 ),*
            );

            // Write result directly to shared memory at output_pos
            let len_bytes = result.len() * 4;
            unsafe {
                let src = result.as_ptr() as *const u8;
                let dst = output_pos as *mut u8;
                dst.copy_from_nonoverlapping(src, len_bytes);
            }

            // Forget the Vec to prevent deallocation (data is in shared memory now)
            core::mem::forget(result);

            len_bytes as i32
        }
    };

    TokenStream::from(expanded)
}
```

### 2.3 Generated Assembly

The macro generates:

```asm
; Input: output_pos in register r0, arg0 in r1, arg1 in r2, arg2 in r3
execute:
    ; Call user function
    bl    user_execute           ; returns pointer to Vec<i32> in r0

    ; Calculate byte length
    ldr   r4, [r0, #8]           ; Vec::len field
    lsl   r4, r4, #2             ; len * 4 (i32 = 4 bytes)

    ; Copy Vec data to shared memory at output_pos
    ldr   r5, [r0, #0]           ; Vec::ptr field
    ldr   r6, [r0, #4]           ; capacity (unused)

    ; memcpy(dst=output_pos, src=r5, len=r4)
    ; (implemented via copy_from_nonoverlapping)

    ; Return length
    mov   r0, r4
    bx    lr
```

## Phase 3: Input Reading Helpers

```rust
// packages/utils/src/accessor.rs

/// Read i32 from shared memory
#[inline]
pub unsafe fn read_i32(ptr: *const i32) -> i32 {
    *ptr
}

/// Read f32 from shared memory (stored as i32 bits)
#[inline]
pub unsafe fn read_f32(ptr: *const i32) -> f32 {
    f32::from_bits(*ptr as u32)
}

/// Read boolean from shared memory
#[inline]
pub unsafe fn read_bool(ptr: *const i32) -> bool {
    *ptr != 0
}

/// Read vec3 (3 f32s) from shared memory
#[inline]
pub unsafe fn read_vec3(ptr: *const i32) -> [f32; 3] {
    let p = ptr as *const f32;
    [p.read(), p.add(1).read(), p.add(2).read()]
}

/// Read slice from shared memory
#[inline]
pub unsafe fn read_i32_slice(ptr: *const i32, len: usize) -> &[i32] {
    std::slice::from_raw_parts(ptr, len)
}

/// Read f32 slice from shared memory
#[inline]
pub unsafe fn read_f32_slice(ptr: *const i32, len: usize) -> &[f32] {
    std::slice::from_raw_parts(ptr as *const f32, len)
}

/// Read with default value
#[inline]
pub unsafe fn read_f32_default(ptr: *const i32, default: f32) -> f32 {
    if ptr.is_null() { default } else { read_f32(ptr) }
}

#[inline]
pub unsafe fn read_i32_default(ptr: *const i32, default: i32) -> i32 {
    if ptr.is_null() { default } else { read_i32(ptr) }
}
```

## Phase 4: Node Implementation Examples

### 4.1 Math Node

```rust
// nodes/max/plantarium/math/src/lib.rs

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(op_type: *const i32, a: *const i32, b: *const i32) -> Vec<i32> {
    use nodarium_utils::{read_i32, read_f32};

    let op = unsafe { read_i32(op_type) };
    let a_val = unsafe { read_f32(a) };
    let b_val = unsafe { read_f32(b) };

    let result = match op {
        0 => a_val + b_val,  // add
        1 => a_val - b_val,  // subtract
        2 => a_val * b_val,  // multiply
        3 => a_val / b_val,  // divide
        _ => 0.0,
    };

    vec![result.to_bits()]
}
```

### 4.2 Vec3 Node

```rust
// nodes/max/plantarium/vec3/src/lib.rs

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(x: *const i32, y: *const i32, z: *const i32) -> Vec<i32> {
    use nodarium_utils::read_f32;

    let x_val = unsafe { read_f32(x) };
    let y_val = unsafe { read_f32(y) };
    let z_val = unsafe { read_f32(z) };

    vec![x_val.to_bits(), y_val.to_bits(), z_val.to_bits()]
}
```

### 4.3 Box Node

```rust
// nodes/max/plantarium/box/src/lib.rs

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(size: *const i32) -> Vec<i32> {
    use nodarium_utils::{read_f32, encode_float, calculate_normals};

    let size = unsafe { read_f32(size) };
    let p = encode_float(size);
    let n = encode_float(-size);

    let mut cube_geometry = vec![
        1,          // 1: geometry
        8,          // 8 vertices
        12,         // 12 faces

        // Face indices
        0, 1, 2, 0, 2, 3,
        0, 3, 4, 4, 5, 0,
        6, 1, 0, 5, 6, 0,
        7, 2, 1, 6, 7, 1,
        2, 7, 3, 3, 7, 4,
        7, 6, 4, 4, 6, 5,

        // Bottom plate
        p, n, n, p, n, p, n, n, p, n, n, n,

        // Top plate
        n, p, n, p, p, n, p, p, p, n, p, p,

        // Normals
        0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];

    calculate_normals(&mut cube_geometry);
    cube_geometry
}
```

### 4.4 Stem Node

```rust
// nodes/max/plantarium/stem/src/lib.rs

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(
    origin: *const i32,
    amount: *const i32,
    length: *const i32,
    thickness: *const i32,
    resolution: *const i32,
) -> Vec<i32> {
    use nodarium_utils::{
        read_vec3, read_i32, read_f32,
        geometry::{create_multiple_paths, wrap_multiple_paths},
    };

    let origin = unsafe { read_vec3(origin) };
    let amount = unsafe { read_i32(amount) } as usize;
    let length = unsafe { read_f32(length) };
    let thickness = unsafe { read_f32(thickness) };
    let resolution = unsafe { read_i32(resolution) } as usize;

    let mut stem_data = create_multiple_paths(amount, resolution, 1);
    let mut stems = wrap_multiple_paths(&mut stem_data);

    for stem in stems.iter_mut() {
        let points = stem.get_points_mut();
        for (i, point) in points.iter_mut().enumerate() {
            let t = i as f32 / (resolution as f32 - 1.0);
            point.x = origin[0];
            point.y = origin[1] + t * length;
            point.z = origin[2];
            point.w = thickness * (1.0 - t);
        }
    }

    stem_data
}
```

## Phase 5: Runtime Implementation

```typescript
// app/src/lib/runtime/memory-manager.ts

export const SHARED_MEMORY = new WebAssembly.Memory({
  initial: 1024, // 64MB initial
  maximum: 4096, // 256MB maximum
});

export class MemoryManager {
  private offset: number = 0;
  private readonly start: number = 0;

  reset() {
    this.offset = this.start;
  }

  alloc(bytes: number): number {
    const pos = this.offset;
    this.offset += bytes;
    return pos;
  }

  readInt32(pos: number): number {
    return new Int32Array(SHARED_MEMORY.buffer)[pos / 4];
  }

  readFloat32(pos: number): number {
    return new Float32Array(SHARED_MEMORY.buffer)[pos / 4];
  }

  readBytes(pos: number, length: number): Uint8Array {
    return new Uint8Array(SHARED_MEMORY.buffer, pos, length);
  }

  getInt32View(): Int32Array {
    return new Int32Array(SHARED_MEMORY.buffer);
  }

  getFloat32View(): Float32Array {
    return new Float32Array(SHARED_MEMORY.buffer);
  }

  getRemaining(): number {
    return SHARED_MEMORY.buffer.byteLength - this.offset;
  }
}
```

```typescript
// app/src/lib/runtime/imports.ts

import { SHARED_MEMORY } from "./memory-manager";

export function createImportObject(nodeId: string): WebAssembly.Imports {
  return {
    env: {
      // Import shared memory
      memory: SHARED_MEMORY,

      // Logging
      __nodarium_log: (ptr: number, len: number) => {
        const msg = new TextDecoder().decode(
          new Uint8Array(SHARED_MEMORY.buffer, ptr, len),
        );
        console.log(`[${nodeId}] ${msg}`);
      },

      __nodarium_log_panic: (ptr: number, len: number) => {
        const msg = new TextDecoder().decode(
          new Uint8Array(SHARED_MEMORY.buffer, ptr, len),
        );
        console.error(`[${nodeId}] PANIC: ${msg}`);
      },
    },
  };
}
```

```typescript
// app/src/lib/runtime/executor.ts

import { SHARED_MEMORY } from "./memory-manager";
import { createImportObject } from "./imports";

export class SharedMemoryRuntimeExecutor implements RuntimeExecutor {
  private memory: MemoryManager;
  private results: Map<string, { pos: number; len: number }> = new Map();
  private instances: Map<string, WebAssembly.Instance> = new Map();

  constructor(private registry: NodeRegistry) {
    this.memory = new MemoryManager();
  }

  async execute(graph: Graph, settings: Record<string, unknown>) {
    this.memory.reset();
    this.results.clear();

    const [outputNode, nodes] = await this.addMetaData(graph);
    const sortedNodes = nodes.sort((a, b) => b.depth - a.depth);

    for (const node of sortedNodes) {
      await this.executeNode(node, settings);
    }

    const result = this.results.get(outputNode.id);
    const view = this.memory.getInt32View();
    return view.subarray(result.pos / 4, result.pos / 4 + result.len / 4);
  }

  private async executeNode(
    node: RuntimeNode,
    settings: Record<string, unknown>,
  ) {
    const def = this.definitionMap.get(node.type)!;
    const inputs = def.inputs || {};
    const inputNames = Object.keys(inputs);

    const outputSize = this.estimateOutputSize(def);
    const outputPos = this.memory.alloc(outputSize);
    const args: number[] = [outputPos];

    for (const inputName of inputNames) {
      const inputDef = inputs[inputName];
      const inputNode = node.state.inputNodes[inputName];
      if (inputNode) {
        const parentResult = this.results.get(inputNode.id)!;
        args.push(parentResult.pos);
        continue;
      }

      const valuePos = this.memory.alloc(16);
      this.writeValue(
        valuePos,
        inputDef,
        node.props?.[inputName] ??
          settings[inputDef.setting ?? ""] ??
          inputDef.value,
      );
      args.push(valuePos);
    }

    let instance = this.instances.get(node.type);
    if (!instance) {
      instance = await this.instantiateNode(node.type);
      this.instances.set(node.type, instance);
    }

    const writtenLen = instance.exports.execute(...args);
    this.results.set(node.id, { pos: outputPos, len: writtenLen });
  }

  private writeValue(pos: number, inputDef: NodeInput, value: unknown) {
    const view = this.memory.getFloat32View();
    const intView = this.memory.getInt32View();

    switch (inputDef.type) {
      case "float":
        view[pos / 4] = value as number;
        break;
      case "integer":
      case "select":
      case "seed":
        intView[pos / 4] = value as number;
        break;
      case "boolean":
        intView[pos / 4] = value ? 1 : 0;
        break;
      case "vec3":
        const arr = value as number[];
        view[pos / 4] = arr[0];
        view[pos / 4 + 1] = arr[1];
        view[pos / 4 + 2] = arr[2];
        break;
    }
  }

  private estimateOutputSize(def: NodeDefinition): number {
    const sizes: Record<string, number> = {
      float: 16,
      integer: 16,
      boolean: 16,
      vec3: 16,
      geometry: 8192,
      path: 4096,
    };
    return sizes[def.outputs?.[0] || "float"] || 64;
  }

  private async instantiateNode(
    nodeType: string,
  ): Promise<WebAssembly.Instance> {
    const wasmBytes = await this.fetchWasm(nodeType);
    const module = await WebAssembly.compile(wasmBytes);
    const importObject = createImportObject(nodeType);
    return WebAssembly.instantiate(module, importObject);
  }
}
```

## Phase 7: Execution Flow Visualization

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Execution Timeline                                │
└─────────────────────────────────────────────────────────────────────────────┘

Step 1: Setup
  SHARED_MEMORY = new WebAssembly.Memory({ initial: 1024 })
  memory.offset = 0

Step 2: Execute Node A (math with 3 inputs)
  outputPos = memory.alloc(16) = 0
  args = [0, ptr_to_op_type, ptr_to_a, ptr_to_b]

  Node A reads:
    *ptr_to_op_type → op
    *ptr_to_a → a
    *ptr_to_b → b

  Node A returns: vec![result.to_bits()]

  Macro writes result directly to SHARED_MEMORY[0..4]
  Returns: 4

  results['A'] = { pos: 0, len: 4 }
  memory.offset = 4

Step 3: Execute Node B (stem with 5 inputs, input[0] from A)
  outputPos = memory.alloc(4096) = 4
  args = [4, results['A'].pos, ptr_to_amount, ptr_to_length, ...]

  Node B reads:
    *results['A'].pos → value from Node A
    *ptr_to_amount → amount
    ...

  Node B returns: stem_data Vec<i32> (1000 elements = 4000 bytes)

  Macro writes stem_data directly to SHARED_MEMORY[4..4004]
  Returns: 4000

  results['B'] = { pos: 4, len: 4000 }
  memory.offset = 4004

Step 4: Execute Node C (output, 1 input from B)
  outputPos = memory.alloc(16) = 4004
  args = [4004, results['B'].pos, results['B'].len]

  Node C reads:
    *results['B'].pos → stem geometry

  Node C returns: vec![1] (identity)
  Macro writes to SHARED_MEMORY[4004..4008]

  results['C'] = { pos: 4004, len: 4 }

Final: Return SHARED_MEMORY[4004..4008] as geometry result
```

## Phase 6: Memory Growth Strategy

```typescript
class MemoryManager {
  alloc(bytes: number): number {
    const required = this.offset + bytes;
    const currentBytes = SHARED_MEMORY.buffer.byteLength;

    if (required > currentBytes) {
      const pagesNeeded = Math.ceil((required - currentBytes) / 65536);
      const success = SHARED_MEMORY.grow(pagesNeeded);

      if (!success) {
        throw new Error(`Out of memory: need ${bytes} bytes`);
      }

      this.int32View = new Int32Array(SHARED_MEMORY.buffer);
      this.float32View = new Float32Array(SHARED_MEMORY.buffer);
    }

    const pos = this.offset;
    this.offset += bytes;
    return pos;
  }
}
```

## Phase 8: Migration Checklist

### Build Configuration

- [ ] Add `--import-memory` to Rust flags in `Cargo.toml`
- [ ] Ensure no nodes export memory

### Runtime

- [ ] Create `SHARED_MEMORY` instance
- [ ] Implement `MemoryManager` with alloc/read/write
- [ ] Create import object factory
- [ ] Implement `SharedMemoryRuntimeExecutor`

### Macro

- [ ] Parse definition JSON
- [ ] Validate function signature (N params, Vec<i32> return)
- [ ] Generate wrapper that writes return value to `output_pos`
- [ ] Add panic hook

### Utilities

- [ ] `read_i32(ptr: *const i32) -> i32`
- [ ] `read_f32(ptr: *const i32) -> f32`
- [ ] `read_bool(ptr: *const i32) -> bool`
- [ ] `read_vec3(ptr: *const i32) -> [f32; 3]`
- [ ] `read_i32_slice(ptr: *const i32, len: usize) -> &[i32]`

### Nodes

- [ ] `float`, `integer`, `boolean` nodes
- [ ] `vec3` node
- [ ] `math` node
- [ ] `random` node
- [ ] `box` node
- [ ] `stem` node
- [ ] `branch` node
- [ ] `instance` node
- [ ] `output` node

## Phase 9: Before vs After

### Before (per-node memory)

```rust
#[nodarium_execute]
pub fn execute(input: &[i32]) -> Vec<i32> {
    let args = split_args(input);
    let a = evaluate_float(args[0]);
    let b = evaluate_float(args[1]);
    vec![(a + b).to_bits()]
}
```

### After (shared memory)

```rust
#[nodarium_execute]
pub fn execute(a: *const i32, b: *const i32) -> Vec<i32> {
    use nodarium_utils::read_f32;
    let a_val = unsafe { read_f32(a) };
    let b_val = unsafe { read_f32(b) };
    vec![(a_val + b_val).to_bits()]
}
```

**Key differences:**

- Parameters are input pointers, not a slice
- Use `read_f32` helper instead of `evaluate_float`
- Macro writes result directly to shared memory
- All nodes share the same memory import

## Phase 10: Benefits

| Aspect            | Before         | After                |
| ----------------- | -------------- | -------------------- |
| Memory            | N × ~1MB heaps | 1 × 64-256MB shared  |
| Cross-node access | Copy via JS    | Direct read          |
| API               | `&[i32]` slice | `*const i32` pointer |
| Validation        | Runtime        | Compile-time         |
