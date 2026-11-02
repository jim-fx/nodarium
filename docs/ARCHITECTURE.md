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

The runtime runs the graph bottom up and hands each node's result to the next node. It doesn't know what the data means, that is up to the node system the nodes belong to (for example [Plantarium](./PLANTARIUM.md)). How nodes and the runtime talk to each other is described in [ABI.md](./ABI.md).

## How are old graphs migrated?

Every graph has a `version`. A graph without one is version 0. When the editor loads a graph, the runtime executes one, or a project is imported, `migrateGraph()` (`app/src/lib/graph-migrations`) runs every migration newer than the graph's version on a copy of it. The migrated graph is where the undo history starts, and the migration is only logged. A graph newer than the app fails to load with an error message.

A change that breaks saved graphs, like renaming a node or an input, gets a migration at the end of `MIGRATIONS` with the next version. Released migrations never change. The helpers rename node types (`renameNodeType`), rename, convert or remove inputs (`renameInputs`, `mapInput`, `removeInputs`) and replace a node with several nodes or a group, with its edges moved onto them (`replaceNodes`). Each new version also gets a fixture in `graph-migrations/fixtures/v<version>.json`: the same plant as the other fixtures, saved in the new format. The tests check that every older fixture migrates to it. The bundled templates are kept at the current version.
