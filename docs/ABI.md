# Node ABI

A node is a WebAssembly module. Any language that compiles to `.wasm` can be used to write one.

The host (the Nodarium runtime) does not know what a node computes. It only:

- reads the node's definition,
- turns the values of input fields into numbers,
- hands each node's result to the next node unchanged.

What a result means (a number, a path, a mesh, ...) is decided by the **node system** the node belongs to, for example [Plantarium](./PLANTARIUM.md) or [Myzel](./MYZEL.md). Nodes of one system share its formats, usually through a shared library. A node never depends on the host to understand its data.

## Data

All data is a list of `i32` words. Pointers are byte offsets into the module's own `memory`, 4-byte aligned.

## Exports

| Export                                | What it does                                                                                                                          |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| `memory`                              | The module's memory.                                                                                                                  |
| `nodarium_alloc(bytes) -> ptr`        | Allocates memory. It stays valid until the next `nodarium_reset`.                                                                     |
| `nodarium_reset()`                    | Frees everything since the last reset.                                                                                                |
| `nodarium_execute(args, argc) -> ptr` | Runs the node. `args` points to `argc` pairs of `[ptr, len]`, one per input. Returns a pointer to `[ptr, len]` pairs, one per output. |

`len` always counts `i32` words. Only `nodarium_alloc` takes bytes.

## Outputs

A node returns one `[ptr, len]` pair for each entry of `outputs` in its definition, in the same order. A node without outputs (like the output node) still returns one pair. The host reads as many pairs as the definition lists, so a node must never return fewer.

Every input that is connected to output `n` of another node receives the value of pair `n`.

## Imports (`env`)

| Import                     | What it does                              |
| -------------------------- | ----------------------------------------- |
| `nodarium_log(ptr, len)`   | Logs `len` bytes of UTF-8 text.           |
| `nodarium_panic(ptr, len)` | Reports an error message before trapping. |

## Definition

The node definition JSON (see [NODE_DEFINITION.md](./NODE_DEFINITION.md)) is stored in a custom section named `nodarium_definition`.

## Input fields

When an input isn't connected, the host passes the value of its field:

| Type                                      | Value                             |
| ----------------------------------------- | --------------------------------- |
| `float`                                   | one word, the bits of an `f32`    |
| `integer`, `seed`                         | one word                          |
| `boolean`                                 | one word, `0` or `1`              |
| `select`                                  | one word, the index of the option |
| `vec3`, `shape`, `curve` and other arrays | `[0, n + 1, ...values, 1, 1]`     |

When it is connected, the node receives exactly what the other node returned.

## Running a graph

1. The host calls `nodarium_reset()` on every module in the graph.
2. It runs the nodes bottom up. For each node it copies the inputs into the module's memory with `nodarium_alloc` and calls `nodarium_execute`.

Results stay in wasm memory until the next reset. The host copies them out only when it has to keep them: the final output, the cache and debug views.

A buffer from `nodarium_alloc` belongs to the module until the next reset. The node may change it and return it, or a part of it, as a result, so the host never reads an input buffer again after `nodarium_execute`. The Rust runtime leaves four words of room before and after every such buffer, a node can return an input with a few words around it without copying it (`abi::frame_input`). It does that only with whole buffers the host allocated for the current call, never with a result of the module, which other nodes may still read. For the same reason a node that decoded such an input may give back the part of it that it doesn't read again before the call ends (`abi::shrink_input`), instead of keeping it next to its copy until the reset.

A result is a buffer of the module as well, it belongs to the module until the next reset. A node that knows how large a result will be before it makes it writes it straight into such a buffer (`abi::write_result`) and returns it, or its first words, instead of building it somewhere else and copying it over. The Myzel nodes that build a mesh count its vertices and faces first and write it with `write_mesh`, so the mesh is in memory once and not a second time as its encoded words. The host sees no difference, a result is a `[ptr, len]` pair either way.

A node that only passes large buffers on, like `join_meshes`, `set_mesh_color` and `output`, keeps the buffers the host allocates for it and the results it writes in place in an arena (`#[nodarium_execute(arena)]`): a region at the end of the memory that `nodarium_reset` rewinds and that grows by the pages it misses. After a reset the allocator holds one free block of the size of the last run. When the input of a slightly larger run still fits into it but the output doesn't, the allocator grows the memory by the whole output, so these nodes grew by about one more mesh than they needed over builds with different seeds. The memory of the module still never shrinks, and the host sees no difference.

The arena and the Rust allocator share the memory, each takes the pages it grows itself. On `wasm32-unknown-unknown` the allocator of `std` is dlmalloc (0.2.10 in Rust 1.90, the crate's `wasm.rs` and `sys_alloc`). Each time it runs out it calls `memory.grow` and gets the new pages for itself, and it adds them to the space it already has only when they start exactly where that space ends (`tbase == Segment::top(sp)`), otherwise they become a space of their own. Newer versions first take the heap the linker leaves inside the initial memory (`__heap_base` to `__heap_end`), which lies below every grown page as well. When the arena grew pages after the allocator, the allocator's next pages start behind them, so it never reaches into the arena, however large an allocation is. `app/src/lib/abi-arena.test.ts` checks this against the compiled `output` node in CI: it fills the arena's last buffer with a marker, makes the node build several megabytes of lines with the allocator, which grows the memory past the arena, and checks that the marker and the input stayed as they were.

So a node with `#[nodarium_execute(arena)]` may allocate as much as it likes while it runs. It must:

- take arena memory only as the buffers the host allocates (`nodarium_alloc`) and the results written in place (`abi::write_result`), and not hold on to it past the reset, which hands it out again;
- not call `memory.grow` itself, and not change the global allocator. Another allocator may assume that the memory up to its end is its own, then it needs the same test before it is used in an arena node.

The arena and the allocator never give memory to each other: after a reset the allocator can't use the pages the arena rewound, and the arena can't use what the allocator freed. A node that mostly builds things with the allocator, and doesn't only pass buffers on, can end up with more memory with the arena than without it, so measure before turning it on.

Older modules that export `execute`, `__alloc` and `__free` instead still work, but are slower and only support a single output.
