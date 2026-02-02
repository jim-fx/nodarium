# Nodarium Architecture

The basic idea of nodarium is to use the advantages of WebAssembly to create a performant, secure and extandable visual programming language.

The visual programming language consists of so called `Nodes` which are stored and distributed as single `.wasm` files. Each `node` receives some arguments and generates some output.

## What is a node?

```typescript
type Node = {
  id: string;
  outputs: string[];
  inputs: {
    [key: string]: NodeInput;
  };
};
```

## How are the arguments defined?

To define which arguments a nodes accepts we use JSON. This json is embeded into the `.wasm` file of our node. An example `definition.json` file could look like this:

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
      "value": 0.5
    }
  }
}
```

For a more in-depth explanation have a look at [./NODE_DEFINITION.md](NODE_DEFINITION.md).

## How are the nodes executed?

## How are the nodes stored?
