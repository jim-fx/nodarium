# Writing a graph by hand

A graph is a JSON file. The quickest start is a template: `pnpm -s nodarium run fern --print-graph > fern.json` (`-s` keeps the lines of pnpm out of the file) prints the fern with every field filled in, `pnpm nodarium nodes` lists the nodes and `pnpm nodarium nodes stem noise` shows their inputs. Outside the repository, `npx @nodarium/runtime` is the same command (see `packages/runtime/README.md`).

A small plant, a stem bent by gravity with eight leaves:

```json
{
  "version": 13,
  "settings": { "resolution.curve": 16 },
  "nodes": [
    { "id": 1, "type": "max/plantarium/stem", "props": { "length": 0.4, "thickness": 0.01 } },
    { "id": 2, "type": "max/plantarium/gravity", "props": { "strength": 0.3 } },
    { "id": 3, "type": "max/plantarium/leaf", "props": { "size": 0.1 } },
    { "id": 4, "type": "max/plantarium/instance", "props": { "amount": 8, "size": 0.5 } },
    { "id": 5, "type": "max/plantarium/output" }
  ],
  "edges": [
    [1, 0, 2, "plant"],
    [2, 0, 4, "plant"],
    [3, 0, 4, "geometry"],
    [4, 0, 5, "input"]
  ]
}
```

- `version` is the version of the graph format, `CURRENT_GRAPH_VERSION` of the runtime (`--print-graph` writes it). A graph without one is migrated from version 0, which renames inputs of today, `check` warns about it.
- A node has an `id`, unique in its graph, its `type` and the values of its inputs in `props`. Inputs left out take the default of the node. `position` is where the node sits in the editor, a node without one is placed in columns when the graph is loaded, `pnpm nodarium format` writes those positions into the file.
- An edge is `[from node, output index, to node, input name]`: output 0 of the stem goes into the input `plant` of gravity. An input takes the type of its socket or one it `accepts`, `nodes` shows both. A number input takes a link too, from `random`, `math`, `curve` or `alpha`, which is how values change along a path or from one stem to the next.
- `settings` are the values of the graph that inputs with a `setting` read, like `resolution.curve` (points along a path), `resolution.circle`, `resolution.field` and the `time.t` of the mushrooms.
- `groups` hold the nodes of a group of the editor, each with its own `nodes` and `edges`. A node of type `__internal/group/instance` with `props.groupId` runs one, the templates of the mushrooms use them. A graph without groups can leave the list out.

Every template is one of three shapes:

- **A plant**: `stem` → `noise`, `gravity`, `branch` and the other nodes that change paths → `output` (`max/plantarium/output`), which makes a tube of every path.
- **A plant with leaves**: a `shape` into `leaf` (or the veins into `leaf_surface`) is the geometry of an `instance`, which places it along the paths, and the `instance` goes into the `output`.
- **A mushroom**: Stipe, Cap and Gills groups, their strands joined into `hyphae_to_mesh`, the meshes into `join_meshes` and `max/myzel/output`. [MYZEL.md](./MYZEL.md) describes the groups, [PLANTARIUM.md](./PLANTARIUM.md) the data of paths, points and instances.

Then the loop: `pnpm nodarium check plant.json` names every mistake with a fix, `pnpm nodarium render plant.json --out screenshots` draws it, `--node 2` draws what comes out of node 2 alone, and `pnpm nodarium run plant.json --set 2.strength=0.6 --timings` tries another value and shows what every node costs.

To look at the graph in the editor, `pnpm nodarium link plant.json` prints a link that opens it in the app as a new project, `--base` points the link at another deployment, like `--base http://localhost:5173` for the dev server. The graph travels in the fragment of the link (`#graph=`), deflated and base64url, it is not sent to the server. A template opens with `/?template=fern`. [AGENT_API.md](./AGENT_API.md) lists the parameters of the address.
