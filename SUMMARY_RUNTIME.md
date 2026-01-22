# Node Compilation and Runtime Execution

## Overview

Nodarium nodes are WebAssembly modules written in Rust. Each node is a compiled WASM binary that exposes a standardized C ABI interface. The system uses procedural macros to generate the necessary boilerplate for node definitions, memory management, and execution.

## Node Compilation

### 1. Node Definition (JSON)

Each node has a `src/input.json` file that defines:

```json
{
  "id": "max/plantarium/stem",
  "meta": { "description": "Creates a stem" },
  "outputs": ["path"],
  "inputs": {
    "origin": { "type": "vec3", "value": [0, 0, 0], "external": true },
    "amount": { "type": "integer", "value": 1, "min": 1, "max": 64 },
    "length": { "type": "float", "value": 5 },
    "thickness": { "type": "float", "value": 0.2 }
  }
}
```

### 2. Procedural Macros

The `nodarium_macros` crate provides two procedural macros:

#### `#[nodarium_execute]`

Transforms a Rust function into a WASM-compatible entry point:

```rust
#[nodarium_execute]
pub fn execute(input: &[i32]) -> Vec<i32> {
    // Node logic here
}
```

The macro generates:
- **C ABI wrapper**: Converts the WASM interface to a standard C FFI
- **`execute` function**: Takes `(ptr: *const i32, len: usize)` and returns `*mut i32`
- **Memory allocation**: `__alloc(len: usize) -> *mut i32` for buffer allocation
- **Memory deallocation**: `__free(ptr: *mut i32, len: usize)` for cleanup
- **Static output buffer**: `OUTPUT_BUFFER` for returning results
- **Panic hook**: Routes panics through `host_log_panic` for debugging
- **Internal logic wrapper**: Wraps the original function

#### `nodarium_definition_file!("path")`

Embeds the node definition JSON into the WASM binary:

```rust
nodarium_definition_file!("src/input.json");
```

Generates:
- **`DEFINITION_DATA`**: Static byte array in `nodarium_definition` section
- **`get_definition_ptr()`**: Returns pointer to definition data
- **`get_definition_len()`**: Returns length of definition data

### 3. Build Process

Nodes are compiled with:
```bash
cargo build --release --target wasm32-unknown-unknown
```

The resulting `.wasm` files are copied to `app/static/nodes/` for serving.

## Node Execution Runtime

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    WebWorker Thread                          │
│  ┌─────────────────────────────────────────────────────────┐│
│  │              WorkerRuntimeExecutor                       ││
│  │  ┌───────────────────────────────────────────────────┐  ││
│  │  │           MemoryRuntimeExecutor                    ││
│  │  │  ┌─────────────────────────────────────────────┐  ││
│  │  │  │  Node Registry (WASM + Definitions)         ││
│  │  │  └─────────────────────────────────────────────┘  ││
│  │  │  ┌─────────────────────────────────────────────┐  ││
│  │  │  │  Execution Engine (Bottom-Up Evaluation)    ││
│  │  │  └─────────────────────────────────────────────┘  ││
│  │  └───────────────────────────────────────────────────┘  ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

### 1. MemoryRuntimeExecutor

The core execution engine in `runtime-executor.ts`:

#### Metadata Collection (`addMetaData`)

1. Load node definitions from registry
2. Build parent/child relationships from graph edges
3. Calculate execution depth via reverse BFS from output node

#### Node Sorting

Nodes are sorted by depth (highest depth first) for bottom-up execution:

```
Depth 3:  n3  n6
Depth 2:  n2  n4  n5
Depth 1:  n1
Depth 0:  Output
Execution order: n3, n6, n2, n4, n5, n1, Output
```

#### Input Collection

For each node, inputs are gathered from:
1. **Connected nodes**: Results from parent nodes in the graph
2. **Node props**: Values stored directly on the node instance
3. **Settings**: Global settings mapped via `setting` property
4. **Defaults**: Values from node definition

#### Input Encoding

Values are encoded as `Int32Array`:
- **Floats**: IEEE 754 bits cast to i32
- **Vectors**: `[0, count, v1, v2, v3, 1, 1]` (nested bracket format)
- **Booleans**: `0` or `1`
- **Integers**: Direct i32 value

#### Caching

Results are cached using:
```typescript
inputHash = `node-${node.id}-${fastHashArrayBuffer(encoded_inputs)}`
```

The cache uses LRU eviction (default size: 50 entries).

### 2. Execution Flow

```typescript
async execute(graph: Graph, settings) {
  // 1. Load definitions and build node relationships
  const [outputNode, nodes] = await this.addMetaData(graph);

  // 2. Sort nodes by depth (bottom-up)
  const sortedNodes = nodes.sort((a, b) => b.depth - a.depth);

  // 3. Execute each node
  for (const node of sortedNodes) {
    const inputs = this.collectInputs(node, settings);
    const encoded = concatEncodedArrays(inputs);
    const result = nodeType.execute(encoded);
    this.results[node.id] = result;
  }

  // 4. Return output node result
  return this.results[outputNode.id];
}
```

### 3. Worker Isolation

`WorkerRuntimeExecutor` runs execution in a WebWorker via Comlink:

```typescript
class WorkerRuntimeExecutor implements RuntimeExecutor {
  private worker = new ComlinkWorker(...);

  async execute(graph, settings) {
    return this.worker.executeGraph(graph, settings);
  }
}
```

The worker backend (`worker-runtime-executor-backend.ts`):
- Creates a single `MemoryRuntimeExecutor` instance
- Manages caching state
- Collects performance metrics

### 4. Remote Execution (Optional)

`RemoteRuntimeExecutor` can execute graphs on a remote server:

```typescript
class RemoteRuntimeExecutor implements RuntimeExecutor {
  async execute(graph, settings) {
    const res = await fetch(this.url, {
      method: "POST",
      body: JSON.stringify({ graph, settings })
    });
    return new Int32Array(await res.arrayBuffer());
  }
}
```

## Data Encoding Format

### Bracket Notation

Inputs and outputs use a nested bracket encoding:

```
[0, count, item1, item2, ..., 1, 1]
   ^    ^       items                ^  ^
   |    |                         |  |
   |    |                         |  +-- closing bracket
   |    +-- number of items + 1   |
   +-- opening bracket (0)         +-- closing bracket (1)
```

### Example Encodings

**Float (5.0)**:
```typescript
encodeFloat(5.0) // → 1084227584 (IEEE 754 bits as i32)
```

**Vec3 ([1, 2, 3])**:
```typescript
[0, 4, encodeFloat(1), encodeFloat(2), encodeFloat(3), 1, 1]
```

**Nested Math Expression**:
```
[0, 3, 0, 2, 0, 3, 0, 0, 0, 3, 7549747, 127, 1, 1, ...]
```

### Decoding Utilities

From `packages/utils/src/tree.rs`:
- `split_args()`: Parses nested bracket arrays into segments
- `evaluate_float()`: Recursively evaluates and decodes float expressions
- `evaluate_int()`: Evaluates integer/math node expressions
- `evaluate_vec3()`: Decodes vec3 arrays

## Geometry Data Format

### Path Data

Paths represent procedural plant structures:

```
[0, count, [0, header_size, node_type, depth, x, y, z, w, ...], 1, 1]
```

Each point has 4 values: x, y, z position + thickness (w).

### Geometry Data

Meshes use a similar format with vertices and face indices.

## Performance Tracking

The runtime collects detailed performance metrics:
- `collect-metadata`: Time to build node graph
- `collected-inputs`: Time to gather inputs
- `encoded-inputs`: Time to encode inputs
- `hash-inputs`: Time to compute cache hash
- `cache-hit`: 1 if cache hit, 0 if miss
- `node/{node_type}`: Time per node execution

## Caching Strategy

### MemoryRuntimeCache

LRU cache implementation:
```typescript
class MemoryRuntimeCache {
  private map = new Map<string, unknown>();
  size: number = 50;

  get(key) { /* move to front */ }
  set(key, value) { /* evict oldest if at capacity */ }
}
```

### IndexDBCache

For persistence across sessions, the registry uses IndexedDB caching.

## Summary

The Nodarium node system works as follows:

1. **Compilation**: Rust functions are decorated with macros that generate C ABI WASM exports
2. **Registration**: Node definitions are embedded in WASM and loaded at runtime
3. **Graph Analysis**: Runtime builds node relationships and execution order
4. **Bottom-Up Execution**: Nodes execute from leaves to output
5. **Caching**: Results are cached per-node-inputs hash for performance
6. **Isolation**: Execution runs in a WebWorker to prevent main thread blocking
