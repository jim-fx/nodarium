# Node ABI

A node is a WebAssembly module. Any language that compiles to `.wasm` can be used to write one.

The host (the Nodarium runtime) does not know what a node computes. It only:

- reads the node's definition,
- turns the values of input fields into numbers,
- hands each node's result to the next node unchanged.

What a result means (a number, a path, a mesh, ...) is decided by the **node system** the node belongs to, for example [Plantarium](./PLANTARIUM.md). Nodes of one system share its formats, usually through a shared library. A node never depends on the host to understand its data.

## Data

All data is a list of `i32` words. Pointers are byte offsets into the module's own `memory`, 4-byte aligned.

## Exports

| Export                                | What it does                                                                                                                          |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| `memory`                              | The module's memory.                                                                                                                  |
| `nodarium_alloc(bytes) -> ptr`        | Allocates memory. It stays valid until the next `nodarium_reset`.                                                                     |
| `nodarium_reset()`                    | Frees everything since the last reset.                                                                                                |
| `nodarium_execute(args, argc) -> ptr` | Runs the node. `args` points to `argc` pairs of `[ptr, len]`, one per input. Returns a pointer to a `[ptr, len]` pair for the result. |

`len` always counts `i32` words. Only `nodarium_alloc` takes bytes.

## Imports (`env`)

| Import                     | What it does                              |
| -------------------------- | ----------------------------------------- |
| `nodarium_log(ptr, len)`   | Logs `len` bytes of UTF-8 text.           |
| `nodarium_panic(ptr, len)` | Reports an error message before trapping. |

## Definition

The node definition JSON (see [NODE_DEFINITION.md](./NODE_DEFINITION.md)) is stored in a custom section named `nodarium_definition`.

## Input fields

When an input isn't connected, the host passes the value of its field:

| Type                             | Value                             |
| -------------------------------- | --------------------------------- |
| `float`                          | one word, the bits of an `f32`    |
| `integer`, `seed`                | one word                          |
| `boolean`                        | one word, `0` or `1`              |
| `select`                         | one word, the index of the option |
| `vec3`, `shape` and other arrays | `[0, n + 1, ...values, 1, 1]`     |

When it is connected, the node receives exactly what the other node returned.

## Running a graph

1. The host calls `nodarium_reset()` on every module in the graph.
2. It runs the nodes bottom up. For each node it copies the inputs into the module's memory with `nodarium_alloc` and calls `nodarium_execute`.

Results stay in wasm memory until the next reset. The host copies them out only when it has to keep them: the final output, the cache and debug views.

Older modules that export `execute`, `__alloc` and `__free` instead still work, but are slower.
