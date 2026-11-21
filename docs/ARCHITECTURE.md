# Nodarium Architecture

The basic idea of nodarium is to use the advantages of WebAssembly to create a performant, secure and extandable visual programming language.

The visual programming language consists of so called `Nodes` which are stored and distributed as single `.wasm` files. Each `node` receives some arguments and generates some output.

## What is a node?

```typescript
type Node = {
  id: string;
  outputs: (string | { type: string; label?: string })[];
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

The runtime runs the graph bottom up and hands each node's result to the next node. It doesn't know what the data means, that is up to the node system the nodes belong to (for example [Plantarium](./PLANTARIUM.md)). How nodes and the runtime talk to each other is described in [ABI.md](./ABI.md). The runtime lives in `packages/runtime` and is published with the compiled nodes as `@nodarium/runtime`, so other projects can generate models without the app (see its [README](../packages/runtime/README.md)).

## Systems

The nodes are grouped into collections, the second part of their id (`max/<collection>/<node>`), each a folder under `nodes/max` and in `app/static/nodes/max`:

| collection       | what                                                                                                                                       | docs                             |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------- |
| `max/core`       | the value nodes: `float`, `math`, `random`, `random_int`, `round`, `curve`, `alpha`, `wave`, `vec3`, `time`, `mix_color`, and the `output` | [CORE.md](./CORE.md)             |
| `max/plantarium` | paths, points, leaves, geometry, instances: the plants                                                                                     | [PLANTARIUM.md](./PLANTARIUM.md) |
| `max/myzel`      | caps, strands, gills, veils, substrates: the mushrooms                                                                                     | [MYZEL.md](./MYZEL.md)           |

All nodes are compatible: Myzel works on the paths of Plantarium, and its surfaces are the same `geometry` as the plant nodes'. A node that both systems use lives in the lower one, the value nodes in Core, `path_info`, `join`, `set_color` and `to_geometry` in Plantarium. There is one of each: since graph version 19 a mushroom is colored by Plantarium's `set_color` too, `hyphae_to_mesh` reads the colors of the points, and the fibres and the fill `set_fibres` and `set_fill` write into them. Core has no data of its own beyond numbers, vectors and colors (see Expressions in [CORE.md](./CORE.md)), so it depends on no system: its crates depend on `nodarium_utils` alone, which also reads the plain values (`read_float`, `read_color`, ...) and mixes the colors.

The one `output` (`max/core/output`) is in Core too, though it takes geometry. Every graph needs it whatever its system, and Core is the collection every graph has. The geometry format is no data of a system: it is in `nodarium_utils::geometry`, which every node builds on, and the output only frames what reaches it, so Core still depends on no system. In Plantarium a mushroom would need Plantarium for its end, and a graph of a third system that makes its own geometry would too. Paths reach the output only through `to_geometry`, which is in Plantarium because it reads paths.

Every graph has Core. The editor loads the collections the graph uses and those of `baseCollections` of the registry, which the app sets to `max/core` (`CORE_COLLECTION` of `@nodarium/types`), first, then every collection `listCollections()` of the registry names, so the add menu offers every node in every graph, under a heading per system, core first. A Cargo crate is named `<collection>_<node>` (`core_random`), `scripts/copy-node-wasm.sh` copies it to `app/static/nodes/max/<collection>/<node>.wasm` by that prefix, and `@nodarium/runtime` ships every collection that folder holds.

## How are old graphs migrated?

Every graph has a `version`. A graph without one is version 0. When the editor loads a graph, the runtime executes one, or a project is imported, `migrateGraph()` (`packages/runtime/src/graph-migrations`) runs every migration newer than the graph's version on a copy of it. The migrated graph is where the undo history starts, and the migration is only logged. A graph newer than the app fails to load with an error message.

A change that breaks saved graphs, like renaming a node or an input, gets a migration at the end of `MIGRATIONS` with the next version. Released migrations never change. The helpers rename node types (`renameNodeType`), rename, convert or remove inputs (`renameInputs`, `mapInput`, `removeInputs`) and replace a node with several nodes or a group, with its edges moved onto them (`replaceNodes`). Each new version also gets a fixture in `graph-migrations/fixtures/v<version>.json`: the same plant as the other fixtures, saved in the new format. The tests check that every older fixture migrates to it. The bundled templates are kept at the current version. A migration that changes what a graph builds, like the one to radians (version 14), is checked on frozen copies of old graphs in `graph-migrations/fixtures/` (`fixtures/radians/` holds the templates as they were in version 13), never on the current templates: tests of the templates only check that each one loads and validates, so changing a template never breaks a test.

## How is the app put together?

`app/src/routes/+page.svelte` wires the parts together. The node editor lives in `app/src/lib/graph-interface`:

- `GraphManager` (`graph-manager.svelte.ts`) holds the graph: its nodes, edges, groups and settings. Every change ends in `commit()`, several changes that belong together run inside `transaction()` and become one undo step and one build. `ChangeRecorder` (`change-recorder.ts`) turns a commit into an undo step of the `HistoryManager` (jsondiffpatch deltas) and a save once the changes rest.
- `GraphState` (`graph-state.svelte.ts`) holds the state of the editor UI: selection, hovered and active node and socket, mouse, clipboard, entering and leaving groups. The camera (position and zoom) is `GraphCamera` in `camera.svelte.ts`.
- Both reach the components of the editor through Svelte context (`createContext` in `graph-state.svelte.ts`), not through props.
- Frames (`frame/Frame.svelte`) are drawn like the WebGL nodes: a quad per frame under the edges, its rectangle derived from the boxes of its nodes on every move, and the title as HTML in the overlay of the nodes, scaled back to 14 px so it stays readable at every zoom. A click on the title selects the frame alone (`GraphState.activeFrameId`), a drag on it moves its nodes. Ctrl+J frames the selection, Alt+P takes nodes out of their frame, Delete removes a selected frame and keeps its nodes. A frame whose last node is deleted or grouped away is gone. The colour is picked in Graph Settings while a frame is selected (`app/src/lib/sidebar/panels/FrameSettings.svelte`), from the six `--color-frame-*` tokens every theme defines; the app hands them to the editor in `GraphOptions.colors`.

`GraphManager` is an event emitter. On `save` the page stores the graph in IndexedDB through the `ProjectManager` (`app/src/lib/project-manager`). On `result` the page runs the graph: `runtime.run(graph, settings)` returns the result together with the debug data and the timings of every node, and the viewer (`app/src/lib/result-viewer`) draws it. The runtime is a `RuntimeExecutor` of `@nodarium/runtime`, in a web worker (`WorkerRuntimeExecutor` in `app/src/lib/runtime`) or, with a debug setting, on the main thread. Nodes come from the registry `createAppRegistry()` makes: a `RemoteNodeRegistry`, which is a `WasmNodeRegistry` of the runtime that fetches the `.wasm` files and caches them in IndexedDB.

The viewport link leads from a part in the viewer to the node that made it and back (General settings → Viewport Link, on by default). Every path carries the creator id of its node, and a geometry with groups and the flag `CREATORS` names the node of each of its vertex groups in a table after the groups (see Geometry in [PLANTARIUM.md](./PLANTARIUM.md)): a geometry without groups is picked by its path, one with groups by the creator of the group under the pointer. A node that makes a geometry of its own, like the substrate of `mesh`, names itself as the creator of its group. An instance is picked by the path it sits on. The paths themselves don't reach the output, `to_geometry` makes them tubes in one geometry with a group per creator, so it writes the links of the paths of its input (id, parent, creator, ...) into a table of that geometry (the flag `PATHS`), and the viewer reads the links from there (`readPathLinks` in `app/src/lib/result-viewer/provenance/part-hit.ts`): a leaf leads to its node through the path it sits on, and a node to the leaves on its paths, as when the paths themselves were in the result. `ViewportLink` (`app/src/lib/provenance/viewport-link.svelte.ts`) turns the selection and the node under the pointer into a `PartHighlight` for the viewer and a hovered part into the node to outline in the editor; `provenance.ts` next to it works out which parts a node stands for, also inside groups (`expandWithChains()` of the runtime). The viewer finds the part under the pointer with an id pass into a target of one pixel (`app/src/lib/result-viewer/provenance/picker.ts`) and tints, dims or hides parts with a chunk added to every material (`highlight.ts`). **I** shows only the parts of the selection.

A debug node shows the output of the node it is connected to in the viewer (`app/src/lib/result-viewer/debug`). Of several debug nodes the viewer shows one: the selected debug node, the one connected to the selected node, otherwise the one connected last (`shown-debug.ts`). Geometry, geometry with groups, instances and the disc of a pattern are drawn like the result, with the material of the viewer and the colours of their vertices, in front of the result, which is dimmed meanwhile; paths and points are lines and dots on top (`debug.ts`). **I** on a debug node, or on a node a debug node is connected to, hides the result and frames what the debug node shows, also a geometry in a frame of its own like a leaf before it is placed.

The help mode (**?**, or Help → Help mode) shows the description of the node or input under the pointer (`components/HelpView.svelte` of the editor). Above it a small animated sketch shows input → output, for every node but those whose name already says it (`UNSKETCHED` in `sketches.ts`, with the reason). The sketches are the app's (`app/src/lib/node-sketches`, the drawing helpers in `kit.ts` and a file per system), the editor gets their component through `GraphOptions.helpSketch`, so it knows no node of a system.

The app uses Svelte 5 runes (`$state`, `$derived`, `$effect`). Maps and sets that the UI reads are `SvelteMap` and `SvelteSet` from `svelte/reactivity`, so changing them updates the UI.

## What are the key types?

They are in `packages/types/src/types.ts`, as Zod schemas with the types inferred from them.

- `Graph`: `id`, `version`, `meta`, `settings`, `nodes`, `edges`, `groups` and the optional `frames`. This is what is saved and what the runtime runs.
- A node in a graph has an `id` (a number), a `type` (the node id, `<author>/<system>/<node>` like `max/plantarium/stem`), a `position`, and `props` with the values of its inputs. In the editor every node also has a `state` that is never saved: its definition, its position while dragged, its mesh.
- An edge is `[fromNode, outputIndex, toNode, inputName]`. Saved, the nodes are ids; in the editor they are the node objects themselves.
- A group (`GroupDefinition`) has its own `nodes` and `edges` and the `inputs` and `outputs` the group node shows. Ctrl+G groups the selected nodes, Tab enters a group. Before a run, the runtime migrates the graph and `expandGroups()` replaces every group node with the nodes inside it.
- A `Frame` is `{ id, label, color?, nodes }`: a label and a colour behind some nodes of one level, the graph and every group have their own `frames`. Only the editor reads them: the rectangle is the box of the nodes, nothing of it is stored, and the runtime, `hashGraph()` and the thumbnails leave frames out, so they never change a build. Frames are optional, a graph without them has no `frames` and no migration was needed.
- A `NodeDefinition` is a node's `definition.json` (see [NODE_DEFINITION.md](./NODE_DEFINITION.md)) plus `execute()`, which takes one value per input and returns one per output.

The types an input or output can have are in `packages/types/src/inputs.ts`: `float`, `integer`, `boolean`, `select`, `seed`, `creator`, `vec3`, `color`, `shape`, `curve`, `geometry`, `path`, `force`, `points`, `pattern`, and `*`, which connects to anything. An input may list more types it takes in `accepts`, `areSocketsCompatible()` decides which output can connect to which input. How each type is encoded is described in [PLANTARIUM.md](./PLANTARIUM.md) and, for the Myzel types, in the module docs of `myzel_core` (`packages/myzel/src`). Graphs before version 15 had a type `mesh` for the Myzel surfaces, their migration makes it `geometry`.

Nodes the app and the runtime bring along have ids under `__internal/`: the group node `__internal/group/instance`, the group's input and output nodes `__internal/group/input` and `__internal/group/output`, and the debug node `__internal/node/debug`.
