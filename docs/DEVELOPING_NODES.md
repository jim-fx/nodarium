# Developing Nodes

This guide writes a Plantarium node in Rust. Nodes can be written in any language that compiles to WebAssembly, as long as they follow the [ABI](./ABI.md). Rust just gets most of it generated.

## Prerequisites

[Rust](https://www.rust-lang.org/tools/install) with the wasm target:

```bash
rustup target add wasm32-unknown-unknown
```

## Create the node

Copy the template and give it a name:

```bash
cp -r nodes/max/plantarium/.template nodes/max/plantarium/cube
```

Then set `name = "plantarium_cube"` in its `Cargo.toml`. Package names must be unique across the Cargo workspace, so every crate is prefixed with its collection (`plantarium_`, `myzel_`). The prefix is stripped again when the `.wasm` is copied into the app, the node id comes from `definition.json`.

## Define the inputs

`src/definition.json` says which inputs the node has and what it returns. See [NODE_DEFINITION.md](./NODE_DEFINITION.md).

```json
{
  "id": "max/plantarium/cube",
  "outputs": ["geometry"],
  "inputs": {
    "size": { "type": "float", "value": 2 }
  }
}
```

## Implement it

`src/lib.rs`:

```rust
use nodarium_macros::{nodarium_definition_file, nodarium_execute};
use nodarium_utils::Ctx;

nodarium_definition_file!("src/definition.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    let mut ctx = Ctx::default();

    // one slice per input, in the order of definition.json
    let size = ctx.float(args[0]);

    // build and return the geometry, see PLANTARIUM.md
    vec![]
}
```

A node with several outputs returns one `Vec<i32>` per output, in the order of `outputs` in the definition, as a tuple, an array or a `Vec<Vec<i32>>`:

```rust
#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> (Vec<i32>, Vec<i32>) {
    (geometry, vec![encode_float(height)])
}
```

`nodarium_definition_file!` checks the definition and embeds it into the `.wasm`. `#[nodarium_execute]` generates the exports the host needs. For a complete example have a look at the `box` node.

## Build

```bash
pnpm build:nodes
```

This builds all nodes and copies them into `app/static/nodes/max/<collection>/<name>.wasm`, so `plantarium_cube.wasm` ends up at `app/static/nodes/max/plantarium/cube.wasm`. Run `pnpm dev` and the node shows up in the app.
