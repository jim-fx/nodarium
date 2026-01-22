# Developing Nodes

This guide will help you developing your first Nodarium Node written in Rust. As an example we will implement a `cylinder` node, which generates a 3D model of a cylinder.

## Prerequesites

You need to have [Rust](https://www.rust-lang.org/tools/install) installed. Rust is the language we are going to develop our node in and cargo compiles our rust code into webassembly.

```bash
# install rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Clone Template

```bash
# copy the template directory
cp -r nodes/max/plantarium/.template nodes/max/plantarium/my-new-node
cd nodes/max/plantarium/my-new-node
```

## Setup Definition

Now we create the definition file of the node.
Here we define what kind of inputs our node will expect and what kind of output it produces. If you want to dive deeper into this topic, have a look at [NODE_DEFINITION.md](./NODE_DEFINITION.md).

`src/definition.json`

```json
{
  "id": "my-name/my-namespace/zylinder-node",
  "outputs": [
    "geometry"
  ],
  "inputs": {
    "height": {
      "type": "float",
      "value": 2
    },
    "radius": {
      "type": "float",
      "value": 0.4
    }
  }
}
```

If we take a look at the `src/lib.rs` file we see that `src/definition.json` is included with the following line:

```rust
include_definition_file!("src/definition.json");
```

This procedural rust macro loads the definition.json, validates its content and embeds it in our output file.

## Implement Node

This is the hardest part when developing a node, for now you can copy the following content into the `src/lib.rs` file:

```rust
use glam::Vec2;
use wasm_bindgen::prelude::*;

nodarium_macros::include_definition_file!("src/definition.json");

#[wasm_bindgen]
pub fn execute(input: &[i32]) -> Vec<i32> {
    let arguments = nodarium_utils::split_args(input);

    let height = nodarium_utils::evaluate_float(arguments[0]);
    let radius = nodarium_utils::evaluate_float(arguments[1]);

    let mut geometry_data = nodarium_utils::geometry::create_geometry_data(16, 16);

    let geometry = nodarium_utils::geometry::wrap_geometry_data(&mut geometry_data);

    // bottom circle
    for i in 0..8 {
        let x = radius * (2.0 * std::f32::consts::PI * i as f32 / 8.0).cos();
        let y = radius * (2.0 * std::f32::consts::PI * i as f32 / 8.0).sin();

        let vec = Vec2::new(x, y).normalize();

        // bottom circle
        geometry.positions[i * 3 + 0] = x;
        geometry.positions[i * 3 + 1] = 0.0;
        geometry.positions[i * 3 + 2] = y;

        geometry.normals[i * 3 + 0] = vec[0];
        geometry.normals[i * 3 + 1] = 0.0;
        geometry.normals[i * 3 + 2] = vec[1];

        // top circle
        geometry.positions[24 + i * 3 + 0] = x;
        geometry.positions[24 + i * 3 + 1] = height;
        geometry.positions[24 + i * 3 + 2] = y;

        geometry.normals[24 + i * 3 + 0] = vec[0];
        geometry.normals[24 + i * 3 + 1] = 0.0;
        geometry.normals[24 + i * 3 + 2] = vec[1];

        geometry.faces[i * 6 + 0] = (i + 8) as i32;
        geometry.faces[i * 6 + 1] = (i as i32 + 1) % 8;
        geometry.faces[i * 6 + 2] = (i) as i32;

        geometry.faces[i * 6 + 3] = 8 + (i % 8) as i32;
        geometry.faces[i * 6 + 4] = 8 + (i as i32 + 1) % 8;
        geometry.faces[i * 6 + 5] = (i as i32 + 1) % 8;
    }

    nodarium_utils::concat_arg_vecs(vec![geometry_data])
}
```

As you can see we import `glam` on the first line. Glam is a fantastic math library. Install it with the following command:

```bash
cargo add glam
```

## Build time

We compile our node by running the following command:

```bash
wasm-pack build --release
```

This will produce a `.wasm` file in the `pkg/` directory of our node. To check if the node works, we can drag it onto the node-graph on https://nodes.max-richter.dev and see if it loads.
