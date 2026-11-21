# Writing a graph by hand

A graph is a JSON file. The quickest start is a template: `pnpm -s nodarium run fern --print-graph > fern.json` (`-s` keeps the lines of pnpm out of the file) prints the fern with every field filled in, `--write fern.json` instead of the `>` writes it into a new file, `pnpm nodarium nodes` lists the nodes and `pnpm nodarium nodes stem noise` shows their inputs. Outside the repository, `npx @nodarium/runtime` is the same command (see `packages/runtime/README.md`), and `nodarium mcp` gives its commands to an agent without a shell as MCP tools.

A small plant, a stem bent by gravity with eight leaves:

```json
{
  "version": 18,
  "settings": { "resolution.curve": 16 },
  "nodes": [
    { "id": 1, "type": "max/plantarium/stem", "props": { "length": 0.4, "thickness": 0.01 } },
    { "id": 2, "type": "max/plantarium/gravity", "props": { "strength": 0.3 } },
    { "id": 3, "type": "max/plantarium/leaf", "props": { "size": 0.1 } },
    { "id": 4, "type": "max/plantarium/instance", "props": { "amount": 8, "size": 0.5 } },
    { "id": 5, "type": "max/plantarium/to_geometry" },
    { "id": 6, "type": "max/core/output" }
  ],
  "edges": [
    [1, 0, 2, "plant"],
    [2, 0, 4, "plant"],
    [3, 0, 4, "geometry"],
    [4, 0, 5, "input"],
    [5, 0, 6, "input"]
  ]
}
```

- `version` is the version of the graph format, `CURRENT_GRAPH_VERSION` of the runtime (`--print-graph` writes it). A graph without one is migrated from version 0, which renames inputs of today, `check` warns about it.
- Every angle is in radians: the `rotation` of `branch`, `instance`, `points_on_path` and `trunk_kink`, every `angle`, the `opening` of `umbel`, the `amount` of `deform` and the curls of `leaf`. 2.3998 is the golden angle (137.5°), 3.1416 half a turn (two ranks, opposite), 1.5708 a right angle. Up to version 13 most of them took degrees and `deform` turns. A graph with an older `version` is converted when it loads: a value set on a node is multiplied by π/180 (2π for `deform`), a `random`, `curve` or `float` that only feeds angles has its range or value scaled, and any other node that feeds an angle keeps its value and gets a `math` node that multiplies it on the way. So an old graph builds the same plant, `run --print-graph` writes it in radians. `roll` and `twist` count whole turns and the `strength` of `gravity` is a share of a right angle, they stay as they were.
- A node has an `id`, unique in its graph, its `type` and the values of its inputs in `props`. Inputs left out take the default of the node. A `select` takes the name of an option or its number (`"op_type": "multiply"` or `2`), `nodes` prints both (`0 add, 1 subtract, 2 multiply, 3 divide`), a boolean `true`, `false`, 1 or 0, a `curve` or `shape` a flat list of numbers, a `text`, like the `rules` of `lsystem`, a string with `\n` between its lines. Lengths, thicknesses and heights are meters, also in a graph whose `meta` says `"unit": "cm"`: the unit only changes what the editor shows. The range `nodes` prints after an input, like `[0..1]`, is the range of its slider in the editor, values outside it work, `check` warns only about one far outside. `position` is where the node sits in the editor, a node without one is placed in columns when the graph is loaded, `pnpm nodarium format` writes those positions into the file.
- An edge is `[from node, output index, to node, input name]`: output 0 of the stem goes into the input `plant` of gravity. An input takes the type of its socket or one it `accepts`, `nodes` shows both. A number input takes a link too, from `random`, `math`, `curve` or `alpha`, which is how values change along a path or from one stem to the next.
- `settings` are the values of the graph that inputs with a `setting` read, like `resolution.curve` (points along a path), `resolution.circle`, `resolution.field` and the `time.t` of the mushrooms.
- `groups` hold the nodes of a group of the editor, each with its own `nodes` and `edges`. A node of type `__internal/group/instance` with `props.groupId` runs one, the templates of the mushrooms use them. A graph without groups can leave the list out.
- `frames` name parts of a graph in the editor: `{ "id": 1, "label": "Leaves", "color": "green", "nodes": [3, 4] }` draws a box labelled "Leaves" around the nodes 3 and 4. The graph and every group have their own list, a frame holds nodes of its own level, each node is in one frame at most, and the box is always the one around its nodes. `color` is one of `gray` (when left out), `green`, `brown`, `blue`, `red` and `violet`, every theme has its own shade of each. Frames change nothing the graph builds, `check` warns about one that names a node that doesn't exist. A graph without frames leaves the list out.

Every graph ends in the one output, `max/core/output`, which shows geometry and instances and takes nothing else. Paths become geometry in a `to_geometry` before it: every path a tube of its thickness, geometry and instances passed on. `check` says so when a path goes into the output. Every template is one of three shapes:

- **A plant**: `stem` → `noise`, `gravity`, `branch` and the other nodes that change paths → `to_geometry`, which makes a tube of every path, → `output`.
- **A plant with leaves**: a `shape` into `leaf` (or the veins into `leaf_surface`) is the geometry of an `instance`, which places it along the paths, and the `instance` goes into the `to_geometry` before the `output`.
- **A mushroom**: Stipe, Cap and Gills groups, their strands joined into `hyphae_to_mesh`, its geometry and the one of the gills joined by `join`, through a `to_geometry` (which passes the geometry on as it is) into the `output`. [MYZEL.md](./MYZEL.md) describes the groups, [PLANTARIUM.md](./PLANTARIUM.md) the data of paths, points and instances.

Then the loop: `pnpm nodarium check plant.json` names every mistake with a fix, `pnpm nodarium render plant.json --out screenshots` draws it, `--node 2` draws what comes out of node 2 alone, `--seeds 1,2,3,4` draws four seeds side by side in one sheet (`--times 0.25,0.5,1` the growth, `--views front` one view), and `pnpm nodarium run plant.json --set 2.strength=0.6 --timings` tries another value and shows what every node costs. `run` prints how many instances every `instance` node placed and how long and wide they are in meters. `--set` also sets an input that an edge goes into (the edge is left out for that run) and a list, `--set 7.curve=0,0,0.5,1,1,1`. When a part comes out too large, missing or the same everywhere, `pnpm nodarium probe plant.json 7` shows what comes out of node 7: for a `random`, `curve`, `math` or `path_info` every value the nodes read from it, where and on how many paths, smallest, largest and mean, for a path or geometry output the paths by depth and the instances with their sizes in meters.

To look at the graph in the editor, `pnpm nodarium link plant.json` prints a link that opens it in the app as a new project, `--base` points the link at another deployment, like `--base http://localhost:5173` for the dev server. The graph travels in the fragment of the link (`#graph=`), deflated and base64url, it is not sent to the server. A template opens with `/?template=fern`. [AGENT_API.md](./AGENT_API.md) lists the parameters of the address.
