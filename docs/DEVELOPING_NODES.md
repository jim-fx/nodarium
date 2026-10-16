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

Then set `name = "cube"` in its `Cargo.toml`.

## Define the inputs

`src/input.json` says which inputs the node has and what it returns. See [NODE_DEFINITION.md](./NODE_DEFINITION.md).

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
use nodarium_utils::evaluate_float;

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(args: &[&[i32]]) -> Vec<i32> {
    // one slice per input, in the order of input.json
    let size = evaluate_float(args[0]);

    // build and return the geometry, see PLANTARIUM.md
    vec![]
}
```

`nodarium_definition_file!` checks the definition and embeds it into the `.wasm`. `#[nodarium_execute]` generates the exports the host needs. For a complete example have a look at the `box` node.

## Build

```bash
pnpm build:nodes
```

This builds all nodes into `app/static/nodes/max/plantarium/`. Run `pnpm dev` and the node shows up in the app.
