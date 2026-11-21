# @nodarium/runtime

Runs a [Nodarium](https://nodes.max-richter.dev) graph without the app and turns its result into vertices and faces. The package brings the compiled nodes (Core, Plantarium and Myzel) with it, so it generates the same plants and mushrooms as the app of the same version. It needs no three.js, the result is plain typed arrays.

The templates of the app come with it as `@nodarium/runtime/templates`. Other graphs are exported from the app and passed in as a file.

## Install

The package is in the npm registry of git.max-richter.dev. Add the scope to the `.npmrc` of your project:

```
@nodarium:registry=https://git.max-richter.dev/api/packages/max/npm/
```

```sh
npm install @nodarium/runtime
```

`latest` is the last release (a tag `vX.Y.Z`). Every push to `main` publishes a prerelease under the dist-tag `dev`, as `<next patch>-dev.<UTC time>.<commit>`, like `0.0.11-dev.202611131200.ac7774d`:

```sh
npm install @nodarium/runtime@dev
```

The dev version in `package.json` stays where it is, `npm update` doesn't move it to a newer dev version or to a release, install `@dev` (or `@latest`) again for that. Right after a release, `dev` is a little older than `latest` until the next push to `main`.

## Templates

`@nodarium/runtime/templates` lists the templates of the project browser of the app, without their graphs. Every template is also a module of its own, `@nodarium/runtime/templates/<id>`, whose default export is the graph. Import one directly when you know which one you need, or `load()` it from the list. Bundlers like Vite put every template into a chunk of its own, the graphs you don't load are not part of your bundle.

```ts
import { createRuntime, decodeResult } from '@nodarium/runtime/node';
import porcini from '@nodarium/runtime/templates/porcini';

const runtime = createRuntime();
const result = await runtime.execute(porcini, { ...porcini.settings, seed: 7 });
const { geometries } = decodeResult(result);
```

```ts
import { loadTemplate, templates } from '@nodarium/runtime/templates';

const mushrooms = templates.filter((t) => t.category === 'Mushrooms');
const graph = await mushrooms[0].load(); // or await loadTemplate('porcini')
```

Every template is a `Graph` of the current graph version, `parseProjectFile()` isn't needed. Its `settings` are the settings of the template in the app: resolutions like `resolution.curve` or `resolution.field`, the `time.t` of most mushrooms and, in a few, a seed. Pass them to `execute()` and put your own seed or settings on top, as above. Settings that are missing take the default of their node. The graphs are shared by everyone who imports them, `structuredClone()` one before you change it.

`unit` is the unit the app shows lengths in. The graph stores meters either way, a unit of `cm` only means that the template is small. Every template but the default plant also says what it is: `species` is the species a plant models (`Nephrolepis exaltata (Boston fern)`), `notes` is one line on how the graph builds it, and `doc` names the guide that describes it in full, `plant-templates` or `myzel` (`nodarium docs <doc>`). The mushrooms have no `species` but the clouded agaric (_Clitocybe nebularis_), their names are their common names.

| id                   | name              | category  | unit |
| -------------------- | ----------------- | --------- | ---- |
| `default-plant`      | Default Plant     | Plants    | m    |
| `fern`               | Fern              | Plants    | cm   |
| `agave`              | Agave             | Plants    | m    |
| `grass`              | Ornamental Grass  | Plants    | cm   |
| `palm`               | Palm              | Plants    | m    |
| `sapling`            | Sapling           | Plants    | m    |
| `lavender`           | Lavender          | Plants    | cm   |
| `sage`               | Sage              | Plants    | cm   |
| `bamboo`             | Bamboo            | Plants    | m    |
| `lady-fern`          | Lady Fern         | Plants    | cm   |
| `maidenhair`         | Maidenhair Fern   | Plants    | cm   |
| `daisy`              | Daisy             | Plants    | cm   |
| `horsetail`          | Field Horsetail   | Plants    | cm   |
| `devils-backbone`    | Devil's Backbone  | Plants    | cm   |
| `knotweed`           | Japanese Knotweed | Plants    | m    |
| `wild-carrot`        | Wild Carrot       | Plants    | cm   |
| `dandelion`          | Dandelion         | Plants    | cm   |
| `yarrow`             | Yarrow            | Plants    | cm   |
| `champignon-groomed` | Champignon        | Mushrooms | cm   |
| `fly-agaric`         | Fly agaric        | Mushrooms | cm   |
| `porcini`            | Porcini           | Mushrooms | cm   |
| `shiitake`           | Shiitake          | Mushrooms | cm   |
| `shaggy-inkcap`      | Shaggy inkcap     | Mushrooms | cm   |
| `chanterelle`        | Chanterelle       | Mushrooms | cm   |
| `oyster`             | Oyster mushroom   | Mushrooms | cm   |
| `honey-fungus`       | Honey fungus      | Mushrooms | cm   |
| `clouded-agaric`     | Clouded agaric    | Mushrooms | cm   |
| `simple-mushroom`    | Simple mushroom   | Mushrooms | cm   |

`sulphur-tuft`, `false-chanterelle` and `bitter-bolete` (Mushrooms, cm) are no longer in that list and the app doesn't show them. They are still part of the package as `unlistedTemplates`, and `loadTemplate()` loads them by their id.

The ids stay, a template that is improved keeps its id. To add a template in this repository, put its graph into `packages/runtime/templates/<id>.json`, add a line `plant(...)` or `mushroom(...)` to the list in `templates/index.ts` and a row to the table above. The app lists it in its project browser from there. Its preview there is a pre-rendered image, see [Template previews](https://git.max-richter.dev/max/nodarium/src/branch/main/app/README.md#template-previews).

### Substrates

The mushroom templates come without a substrate. `withSubstrate(graph, substrate)` returns a copy of a mushroom graph that grows on one: a Substrate group with the `max/myzel/mesh` node joins its scan into the last input of the `join` before the `to_geometry` and the `output`, so it is the highest material group of the geometry, and gives the places of `sample_mesh` on it to the `points` of the Stipe group. Its instance is the one node with a `substrate` prop. `substrates` keeps the substrate `porcini`, `chanterelle`, `honey-fungus`, `sulphur-tuft`, `false-chanterelle` and `bitter-bolete` grew on before, `loadTemplate(id, { substrate: true })` loads a template with it.

```ts
import { loadTemplate, substrates, withSubstrate } from '@nodarium/runtime/templates';

const honeyFungus = await loadTemplate('honey-fungus', { substrate: true });
const porcini = withSubstrate(await loadTemplate('porcini'), {
  ...substrates.porcini!,
  kind: 'stump'
});
```

## Export a project from the app

Open the plant or mushroom in the app and choose **File → Export .json**. The app saves it as `<title>.nodarium.json`. `parseProjectFile()` reads these files, and the bare graphs older versions of the app saved. It checks them and migrates them to the format of the runtime. A graph made with a newer version of the app than the package throws a `GraphVersionError`.

`graphFileJsonSchema()` gives the JSON Schema of what `parseProjectFile()` reads, a project file or a bare graph; the site serves it as [/graph.schema.json](https://nodes.max-richter.dev/graph.schema.json), and the templates point to it with `"$schema"`, so an editor checks a graph while it is written. The schema says what the file looks like, `validateGraph()` below whether its nodes and edges make sense.

## Check a graph

A graph written by hand or by a program can have mistakes the runtime doesn't name: an edge into an input the node doesn't have is left out, a prop with a typo is never read. `validateGraph()` finds all of them at once, in the graph and in its groups, without running anything. It takes the graph and the definitions of the nodes it may use by their id, the internal nodes (groups and the debug node) are known without them. It returns a list of `GraphIssue`, empty for a graph without mistakes.

```ts
import { FileNodeRegistry, validateGraph } from '@nodarium/runtime/node';

// the nodes the graph may use, a type that doesn't load is reported by the validation
const registry = new FileNodeRegistry();
await Promise.allSettled(graph.nodes.map((node) => registry.load([node.type])));
const definitions = new Map(registry.getAllNodes().map((d) => [d.id, d]));
for (const issue of validateGraph(graph, definitions)) {
  console.log(issue.severity, issue.code, issue.message);
  // error unknown-input noise#2 has no input "strenght", did you mean "strength"?
}
```

```ts
type GraphIssue = {
  severity: 'error' | 'warning';
  code: GraphIssueCode;
  message: string; // names the node as type#id, the input and the fix where there is one
  nodeId?: number; // the node, inside the group groupId if that is set
  groupId?: number;
  edge?: SerializedEdge; // [from node, output, to node, input]
  suggestion?: string; // what to write instead: a name, a node type or a value
};
```

| code                    | severity | what is wrong                                                                        |
| ----------------------- | -------- | ------------------------------------------------------------------------------------ |
| `unknown-node-type`     | error    | no definition for the type of a node, or a group instance of a group that is missing |
| `unknown-input`         | error    | an edge goes into an input the node doesn't have, or a shown input doesn't exist     |
| `unknown-prop`          | error    | a prop the node has no input for, or one it never reads (a seed or a setting)        |
| `incompatible-edge`     | error    | the type of the output doesn't fit the input, or the input can't be connected        |
| `missing-node`          | error    | an edge from or to a node the graph doesn't have                                     |
| `output-out-of-range`   | error    | an edge reads an output the node doesn't have                                        |
| `duplicate-id`          | error    | two nodes or two groups with the same id                                             |
| `input-connected-twice` | error    | two edges into one input, only the last counts                                       |
| `no-output`             | error    | the graph has no output node                                                         |
| `several-outputs`       | error    | the graph has more than one output node, only the first is used                      |
| `cycle`                 | error    | nodes that depend on each other in a circle, or a group that contains itself         |
| `wrong-value-type`      | error    | a prop of the wrong type, like a list for a float                                    |
| `value-out-of-range`    | warning  | a number far outside the range of its input, or a select option that doesn't exist   |
| `unknown-setting`       | warning  | a setting of the graph no node reads                                                 |
| `no-version`            | warning  | the graph has no `version` and is migrated from version 0                            |
| `unreachable-node`      | warning  | a node that leads neither to the output nor to a debug node, it never runs           |

Names are compared by their edit distance, a typo gets a "did you mean", an unknown node type the nearest ids of the definitions. An edge fits when the type of the output is the type of the input or one it `accepts`, the same rule the editor connects by (`inputAccepts()`). The editor lets a number be typed past the min and max of its input, so only a value further outside than 100 times the width of the range is a warning. A prop under an edge is left alone, the edge wins.

A graph of an older version is checked as it is after its migration. A graph without a `version` is checked as it is written, with the warning `no-version`: without the field it would be migrated from version 0, which renames inputs of today.

`createRuntime({ validate: true })` checks every graph before it runs: a graph with an error throws a `GraphValidationError`, its `issues` list them all, warnings included. Graphs with only warnings run. It costs a fraction of a millisecond per run (0.2 ms for the largest template, 140 nodes), `execute()` and `run()` without the option check nothing.

```ts
import { createRuntime, GraphValidationError } from '@nodarium/runtime/node';

const runtime = createRuntime({ validate: true });
try {
  await runtime.execute(graph, graph.settings);
} catch (error) {
  if (error instanceof GraphValidationError) console.log(error.issues);
}
```

## The nodarium command

The package has a command line, `nodarium`, to look at the nodes and at a graph without writing code. It needs Node 20 or later and nothing else:

```sh
npx @nodarium/runtime nodes stem
npx @nodarium/runtime render my-plant.json --out .
```

In this repository it is `pnpm nodarium` in the root, with the nodes of `app/static/nodes`. Every command takes a template id (`fern`, `porcini`, ...), the path of a graph or of a `.nodarium.json` project, a link of the app, or `-` for stdin, prints JSON with `--json` and exits with 1 on an error.

| command                        | does                                                                                                       |
| ------------------------------ | ---------------------------------------------------------------------------------------------------------- |
| `nodes [name...] [--type t]`   | every node with its signature and the first sentence of its description, or the inputs of named nodes      |
| `check <graph...>`             | the issues of `validateGraph()`, see [Check a graph](#check-a-graph)                                       |
| `run <graph...>`               | builds the graph, prints vertices, faces, instances and its bounds in meters                               |
| `probe <graph> <node id>`      | what comes out of one node: the values the nodes read from it, or its paths, instances and geometry        |
| `render <graph...>`            | front, side and top view in one PNG, `<out>/<name>.png`, and a line with its size                          |
| `format <graph...> [--check]`  | places the nodes without a position in columns, like loading the graph does, and writes them into the file |
| `link <graph...> [--base url]` | prints a link that opens the graph in the app as a new project, `https://nodes.max-richter.dev` by default |
| `docs [topic]`                 | the guides that come with the package, see [Docs](#docs); a list of them without a topic                   |
| `mcp`                          | starts an MCP server on stdio with these commands as tools, see [MCP server](#mcp-server)                  |

`run`, `probe` and `render` check the graph first: its warnings go to stderr, a graph with an error doesn't run. They take `--seed n` (1 by default), `--time t` for the setting `time.t` and `--set key=value`, any number of times: a key with a dot after a node id sets an input of that node (`--set 4.length=0.3`), any other key a setting (`--set resolution.curve=8`). A value is read as JSON, numbers separated by commas are a list (`--set 77.curve=0,0,1,1`) and a word stays a word, like the name of an option (`--set 6.face=path`). Setting an input that an edge goes into takes the edge out for that run, a line on stderr says which. The options of a select input can be given by their name or their number in a graph, `nodes` prints both. `run` prints a line for every `instance` and `instance_from_points` node: how many instances it placed and how long and wide they are in meters (`instancers` with `--json`), so a size doesn't have to be worked out from the thickness of a path. `run --timings` adds the ms and the output bytes of every node, `run --print-graph` prints the graph with a `version` and a position for every node instead of running it, `--write file` writes it into a file instead (`run fern --print-graph --write fern.json`). `--write` makes a new file, for one graph; it refuses a file that exists unless `--overwrite` is given. `render` takes `--out dir` (`./screenshots`), `--name name` for the name of the PNG (`<name>.png`, the name of the graph by default, only with one graph), `--size px` (512 per view), `--views` for the views and their order (`--views front` or `--views side,top`, out of `front`, `side`, `top` and `below`), `--focus x,z,r` for a close-up, `--below` for a view from below after the others, and `--node id[:output]` to draw what comes out of one node: a debug node behind it in a copy of the graph shows its geometry, its paths as tubes of their thickness and its points as dots. `--nodes dir` reads the nodes from another folder.

`probe <graph> <node id>` runs the graph with the outputs of one node tapped. For a float or integer output, like the one of `random`, `curve`, `math` or `path_info`, it prints every value a node read from it, by the edge it came through (`into instance#45 size`) and the node that read it when that is another one (`read in instance#46` for a `math` in between): how many, on how many paths, the smallest, the largest and the mean. A value set on a node is printed as it is. For a path, geometry or points output it prints the paths by depth with their lengths in meters, points and growth, every block of instances with the length and width of its instances in meters, and the extents of the geometry. It works on the nodes of the graph itself, not those inside a group.

```
$ nodarium probe daisy 37
daisy: random_int#37
  output 0 (integer): read 4 times
    into instance#30 amount: 4 values on 4 paths, 20 to 26 (mean 23)
```

`render --seeds` and `--times` draw a contact sheet instead, `<name>-sheet.png`: the graph built once per seed and time, a column per seed and a row per time, all at the same scale and with 256 pixels per view unless `--size` says otherwise. Every cell is labelled `s<seed> t<time>` in its top left corner. A line of JSON after the line of the PNG (or the line of `--json`) says which cell is which, with its place in pixels and its numbers; a cell that built nothing is `"empty": true`.

```sh
$ nodarium render fern --seeds 1,2,3 --times 0.3,1 --views front,top
fern: /home/me/screenshots/fern-sheet.png, a sheet of 3 × 2 cells
{"cells":[{"seed":1,"time":0.3,"column":0,"row":0,"x":0,"y":0,"triangles":94116,...},...]}
```

### A root folder

`--root dir` keeps the command inside one folder, for an agent that may only work there. Every path the command is given is resolved against `dir`: the graphs it reads, `--out`, `--write` and the PNG `--name` makes. A path that leads outside, with `..`, as an absolute path elsewhere or through a symbolic link inside the folder that points out, ends the command with one line on stderr and exit code 1, as does a link that points to nothing (a write would follow it). Template ids, links of the app and `-` for stdin work as without it. `--nodes` and `mcp` are refused under `--root`: the nodes are code the command runs, they are those of the package, and the MCP server takes no paths. `--root` is given once, a second one is refused, so a wrapper that puts it in front of the arguments of an agent can't be widened by them.

```sh
$ nodarium --root work run fern --print-graph --write fern.json
fern: wrote /home/me/work/fern.json
$ nodarium --root work render ../secret.json
../secret.json: error ../secret.json is outside the root /home/me/work
```

### Docs

`nodarium docs` lists the guides that come with the package, `nodarium docs <topic>` prints one as markdown: `graphs` (how to write a graph by hand, [docs/GRAPHS.md](https://git.max-richter.dev/max/nodarium/src/branch/main/docs/GRAPHS.md)), `plant-templates` (what the plant nodes can express and how every plant template is built), `core` (the value nodes every graph has), `plantarium` (the data of the plant nodes), `myzel` (the mushroom nodes and their workflow) and `templates`, the list of templates with their species and how each is built. A topic can also be named by its file, `PLANTARIUM.md`. A link from one guide to another points into the repository, followed by the `nodarium docs` command of the guide where the package has it. The guides name the commands of the repository, `pnpm nodarium ...`, which are `nodarium ...` here.

`link` checks the graph like `run` and prints a link like `https://nodes.max-richter.dev/#graph=<data>`. `<data>` is the JSON of the graph, deflated and base64url; it is in the fragment, so the server never sees it. The app opens it as a new project, or the project that already has the same graph. `--base` points it at another deployment, a preview or `http://localhost:5173`. The other way round, a link given to `check`, `run` or `render` is read like a file. In code, `graphLink(graph, base)` makes a link, `graphLinkData(link)` takes the data out of one and `decodeGraph(data)` gives the JSON back, `encodeGraph(graph)` is the data alone. They are in the main entry and run in the browser and in Node; `decodeGraph` stops at `MAX_GRAPH_LINK_BYTES` (1 MB) of JSON.

`nodarium nodes` lists the nodes under a heading per system, `# core`, `# plantarium` and `# myzel`, and writes a node like this, `nodes <name>` adds the description of every input:

```
max/plantarium/noise (path) -> path
  Bends paths with smooth noise, more toward the tip. Every segment keeps its length, the same seed gives the same bend.
  plant: path
  scale: float = 1 [0.1..10]
  strength: float = 0.5 [0..1]
  seed: seed
  directional_strength: vec3 = [1, 1, 1] (hidden)
  octaves: integer = 1 [1..5] (hidden)
```

In parentheses are the types of the data a node takes, after the arrow what it gives. An input shows its type, its default, its range and a unit, `hidden` ones are set in the templates, `not connectable` ones only on the node, and those with `setting` read a setting of the graph.

The same is in the package: `listNodes()` gives the definitions of all nodes of the package (or of a folder), `formatNode()` and `formatNodeList()` write them as above, `nodeSummary()` gives the id, the system, the data types and the first sentence of a node as an object, `nodeSystem(id)` the system of a node and `groupBySystem()` the nodes by system, in the order of `SYSTEMS`, and `renderToPng(result, options)` rasterizes a result into a PNG with the size of what it shows, without any dependency but `node:zlib`; `options.views` picks the views (`['front']`), the default draws front, side and top as before. `renderSheetToPng(cells, options)` draws several results into one sheet at the same scale, `columns` cells in a row, each with an optional `label`. `readGraphSource(target)` reads a graph as the commands do, from a template id, a file, a link of the app or `-`, `parseSets()` and `applySets()` read and apply `--set`. All of them are in `@nodarium/runtime/node`.

```ts
import { createRuntime, listNodes, renderToPng } from '@nodarium/runtime/node';
import fern from '@nodarium/runtime/templates/fern';
import { writeFile } from 'node:fs/promises';

const nodes = await listNodes();
const result = await createRuntime().execute(fern, { ...fern.settings, seed: 1 });
const rendered = renderToPng(result, { size: 256 }); // null when there is nothing to draw
if (rendered) await writeFile('fern.png', rendered.png);
console.log(nodes.length, rendered?.stats.height);
```

## MCP server

An agent in a chat app like Claude Desktop has no shell to run `nodarium` in. `nodarium mcp` gives it the same commands as the tools of an [MCP](https://modelcontextprotocol.io) server on stdio, so it can be asked for "a fern with wider fronds" and answer with a picture and a link. It needs the MCP SDK, `@modelcontextprotocol/sdk` 1.32 or later, an optional peer dependency that only this command loads. Without it, `nodarium mcp` stops with a line that says what to install.

Add the scope of the registry to `~/.npmrc` as in [Install](#install), then the server to the config of the client, for Claude Desktop `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "nodarium": {
      "command": "npx",
      "args": [
        "-y",
        "-p",
        "@nodarium/runtime",
        "-p",
        "@modelcontextprotocol/sdk",
        "nodarium",
        "mcp"
      ]
    }
  }
}
```

`npx` installs both packages once and starts the command of the first. With the packages installed in a project, the command is the path of `node_modules/.bin/nodarium` and the arguments are `["mcp"]`. In this repository, `pnpm nodarium mcp` serves the nodes of `app/static/nodes`.

The instructions of the server tell the agent the order of the tools and how the three systems fit together: Core has the value nodes, Plantarium the paths of plants, Myzel grows mushrooms on those paths, every graph ends in the one output `max/core/output`, which takes only geometry, paths reach it through `to_geometry`, and there is one `set_color`.

| tool                                                                                           | answers with                                                                                                                                                                                                                                                    |
| ---------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `list_nodes(query?, type?)`                                                                    | the list of `nodarium nodes`, by system; `type` as `--type`, `query` keeps the nodes whose id or description contains it                                                                                                                                        |
| `get_node(id)`                                                                                 | `nodarium nodes <id>`: the node with every input and its description; several ids separated by spaces                                                                                                                                                           |
| `list_templates()`                                                                             | the templates as `id: name (category, unit)`, below each its species and how it is built (`nodarium docs templates`)                                                                                                                                            |
| `get_template(id)`                                                                             | the graph of a template as `nodarium run <id> --print-graph` prints it                                                                                                                                                                                          |
| `check_graph(graph)`                                                                           | `nodarium check`: the issues of `validateGraph()`, one per line                                                                                                                                                                                                 |
| `run_graph(graph, seed?, settings?)`                                                           | `nodarium run --timings --json`: vertices, faces, bounds, the instances of every instance node, and the ms and bytes of every node                                                                                                                              |
| `probe_node(graph, node, seed?, settings?)`                                                    | `nodarium probe`: the values the nodes read from one node, or its paths, instances and geometry with their sizes                                                                                                                                                |
| `render_graph(graph, seed?, settings?, node?, output?, views?, size?, focus?, seeds?, times?)` | the PNG of `nodarium render` as an image, front, side and top view (`views: "front-side-top-below"` adds one from below, a list like `["front"]` picks them), and its numbers; `seeds` and `times` draw a sheet of at most 16 cells, as `--seeds` and `--times` |
| `graph_link(graph)`                                                                            | the link of `nodarium link`                                                                                                                                                                                                                                     |
| `get_docs(topic?)`                                                                             | `nodarium docs`: the list of the guides, or one of them                                                                                                                                                                                                         |

`graph` is the JSON of a graph or a project file, its JSON as text, a template id or a link of the app. `settings` are put on top of those of the graph like `--set resolution.curve=8`, `seed` is 1 by default. A tool and its command give the same answer for the same graph. A graph with an error doesn't run: `run_graph`, `render_graph` and `graph_link` answer with an error that lists all its issues, so the model can fix its graph. Warnings come after the answer. The prompt `write_graph` tells a model in which order to use the tools, followed by [docs/GRAPHS.md](https://git.max-richter.dev/max/nodarium/src/branch/main/docs/GRAPHS.md); its argument `request` is what to make. The guides are a tool, `get_docs`, not resources of MCP: a client attaches resources when its user picks them, a tool the model calls itself when it needs a guide.

What the server may reach: it reads the nodes and the templates of the package, and nothing else; `--nodes dir` in the config points it at other nodes. `nodarium --root dir mcp` is refused, the server reads and writes no files a root could keep. The tools take a graph, never the path of a file, and write no files, `render_graph` answers with the PNG itself. A link of the app is decoded where it is, nothing is fetched. A graph may have 1 MB of JSON (`MAX_GRAPH_LINK_BYTES`, what a link may unpack to), a view of `render_graph` at most 1024 pixels. Stdout carries only the protocol: the logs of the runtime and of the nodes go to stderr. A mushroom takes a few seconds, during which the server answers nothing else.

## Graphs without positions

`position` of a node is optional. A graph written by hand or by a program can leave it out, `placeNodes()` places such nodes in columns by their depth, the editor does that when it loads the graph. [docs/GRAPHS.md](https://git.max-richter.dev/max/nodarium/src/branch/main/docs/GRAPHS.md) explains how to write a graph.

## Node

`@nodarium/runtime/node` reads the nodes from the package, `createRuntime()` needs no arguments:

```ts
import { createRuntime, decodeResult, parseProjectFile } from '@nodarium/runtime/node';
import { readFile } from 'node:fs/promises';

const { graph } = parseProjectFile(JSON.parse(await readFile('fern.nodarium.json', 'utf8')));

const runtime = createRuntime();
const result = await runtime.execute(graph, { ...graph.settings, seed: 7 });
const { geometries, instances, vertexCount, faceCount } = decodeResult(result);
```

`graph.settings` are the settings of the project as the app shows them, with the seed. Settings that are missing take the default of their node.

To read the nodes from somewhere else, pass a registry: `createRuntime({ registry: new FileNodeRegistry('/path/to/nodes') })`. `nodesDir` is the folder of the nodes in the package.

## Browser

The main entry `@nodarium/runtime` has no Node imports. The browser fetches the nodes, so serve the `nodes` folder of the package and point a `FetchNodeRegistry` at it:

```ts
import {
  createRuntime,
  decodeResult,
  FetchNodeRegistry,
  parseProjectFile
} from '@nodarium/runtime';

const runtime = createRuntime({ registry: new FetchNodeRegistry('/nodes') });

const file = await fetch('/models/champignon.nodarium.json').then(r => r.json());
const { graph } = parseProjectFile(file);
const { geometries } = decodeResult(await runtime.execute(graph, graph.settings));
```

The registry fetches `<baseUrl>/<user>/<collection>/<node>.wasm`, like `/nodes/max/core/output.wasm`. In a SvelteKit or Vite project, copy the folder into `static` (or `public`) once it is installed, for example with a `postinstall` script:

```json
{
  "scripts": {
    "postinstall": "rm -rf static/nodes && cp -R node_modules/@nodarium/runtime/nodes static/nodes"
  }
}
```

Copy them again after every update of the package, the nodes have to match the runtime.

Since 2.0 the value nodes (`float`, `math`, `random`, `random_int`, `round`, `curve`, `alpha`, `wave`, `vec3`, `time` and `mix_color`) are a collection of their own, `max/core` (`CORE_COLLECTION` of the package), in `nodes/max/core`. Graphs of version 15 and before name them `max/plantarium/...` and `max/myzel/...` and are migrated when they load, a graph written with version 16 or later uses `max/core/...`.

A mushroom takes seconds to build, plants are much faster. To keep the page responsive, run the runtime in a Web Worker and transfer `result.buffer` back to the page. The nodes are compiled synchronously when they are registered, the largest is below 200 KB, the app compiles them on the main thread as well.

On the server (SvelteKit `+page.server.ts`, an API route), use `@nodarium/runtime/node`. It finds the nodes through `import.meta.url`, so leave the package external in the SSR build (the Vite default) or pass the folder to `FileNodeRegistry` yourself.

## The result

`execute()` returns one `Int32Array`. `decodeResult()` splits it into typed arrays. They are views into the result, copy them before you change or transfer it.

```ts
type DecodedResult = {
  geometries: GeometryData[]; // the tubes of a plant, the surface of a mushroom, both with groups
  instances: InstancesData[]; // Plantarium instances like leaves
  vertexCount: number; // of everything, every instance counted
  faceCount: number;
};

type MeshData = {
  vertexCount: number;
  faceCount: number;
  faces: Int32Array; // three vertex indices per face
  positions: Float32Array; // x, y, z per vertex
  normals: Float32Array; // x, y, z per vertex
  colors?: Uint8Array; // RGBA8 per vertex in sRGB, alpha how much of it shows over the material
  groups?: Int32Array; // the material group (tissue) of every vertex, none: one group
  creators?: Int32Array; // by group, the creator id of the node that made it, NO_CREATOR for none
};

type GeometryData = MeshData & {
  pathId: number; // the path the geometry was made from, NO_PATH for none
  paths?: Int32Array; // the links of the paths of a to_geometry, readPathLinkTable() reads them
};

type InstancesData = MeshData & {
  instanceCount: number;
  creator: number; // the creator id of the node that placed the instances, NO_CREATOR for none
  matrices: Float32Array; // a column-major 4x4 matrix per instance
  pathIds: Int32Array; // the path every instance sits on
};
```

Every graph ends in the one output, `max/core/output`, which takes only geometry: a plant comes out of its `to_geometry` as one geometry of tubes with a group for every node that made them, and `instances`, a mushroom as one geometry with `groups`. The tubes carry the links of their paths in `paths` (id, parent, creator, …, see `readPathLinkTable()` of `@nodarium/utils`), an instance names the path it sits on in `pathIds` and the node that placed it in `creator`. Its colors come from nodes like `set_color`, and its group for every vertex lets a viewer give every tissue its own material. Alpha is how much of a color shows over the material: vertices without a color of their own have alpha 0 and take the color of the material.

Up to version 1.2 mushrooms came out as `meshes`, read by `readMesh()`, and the graphs of version 14 and before had nodes and links of the type `mesh`. Since 2.0 they are `geometries` with `groups`: read a mushroom with `decodeResult(result).geometries` or `readGeometry()`, the flags of the header are `GEOMETRY_FLAGS`. Old graphs are migrated when they load.

Up to graph version 17 Plantarium and Myzel had an output node each, `max/plantarium/output` and `max/myzel/output`, and the result held the paths, a geometry for every path and Myzel lines. Since graph version 18 there is one, `max/core/output`, and a `to_geometry` before it turns the paths into tubes: the result holds no paths and no lines, the tubes of a plant are one geometry with groups, and code that read the path links from the paths of the result reads them from `paths` of that geometry. A graph written by hand ends `… → max/plantarium/to_geometry → max/core/output`, `validateGraph` suggests the `to_geometry` when paths go into the output. Saved graphs are migrated when they load.

Up to graph version 18 Myzel had a `set_color` of its own, `max/myzel/set_color`, that put the color of the strands next to them in the list of paths. Since graph version 19 `max/plantarium/set_color` colors strands like any other path, a color at every point, and `hyphae_to_mesh` colors the surface around them with it, so a curve of `alpha` makes a gradient along a mushroom too. The migration makes every Myzel `set_color` a Plantarium one with `depth` 10, so it colors every level as before. A graph written by hand with version 19 uses `max/plantarium/set_color` with the input `plant`.

Up to 1.2, `set_fibres` and `set_fill` put their value next to the strands in the list of paths, as a hidden item for every path of the nodes that made them. Since then they write it into every point of the paths that pass through them, like `set_color` its color, and `hyphae_to_mesh` reads it there, so a curve of `alpha` into `fibres` or `fill` changes them along a strand. The graphs stay as they are. A path carries what it has at its points in its type, a sum of `POINT_VALUES` of `@nodarium/utils` (`colors` 16, `fibres` 32, `fill` 64): `isPath()`, `pathPointCount()` and `readPathColors()` take every one of them. `sortResultItems()` gives `geometries` and `instances` only, the Myzel lines are gone. The geometry of the substrate (`max/myzel/mesh`) names its node as the creator of its group, so a viewer leads from it to that node.

With three.js, a geometry becomes a `BufferGeometry`:

```ts
import { BufferAttribute, BufferGeometry } from 'three';

function toGeometry({ faces, positions, normals, colors }: MeshData) {
  const geometry = new BufferGeometry();
  geometry.setIndex(new BufferAttribute(new Uint32Array(faces), 1));
  geometry.setAttribute('position', new BufferAttribute(positions.slice(), 3));
  geometry.setAttribute('normal', new BufferAttribute(normals.slice(), 3));
  if (colors) geometry.setAttribute('color', new BufferAttribute(colors.slice(), 4, true));
  return geometry;
}
```

The matrices of `instances` fit `InstancedMesh.instanceMatrix` as they are. Three.js treats colors as linear, convert them from sRGB if the shades matter.

## More

- `cache: true` in `createRuntime()` keeps the results of the nodes, so a run with a changed setting or seed only runs the nodes the change reaches. It keeps the results by the time they save per byte: a result over 1 MB that took less than 1 ms per MB to make, like a mesh a node only passes on, is made again instead. One time can be twice too slow, so a result between 1 and 2 ms per MB is kept once it was made a second time and the faster of its times still reaches 1 ms per MB. Which results the same graph keeps then doesn't depend on one slow run.
- A wasm memory never shrinks, so after every run the executor releases the instances of the nodes that left the graph. A process only has room for about 125 wasm memories, so once it holds more than `keepInstances` (40) it releases the biggest ones, those outside the latest graph first. `releaseInstancesOver` releases every instance with more bytes after every run (`Infinity` by default). When runs overlap, the last one to finish releases. A released node instantiates its compiled module again when it runs next. `releaseInstances()` releases all of them, for a graph that is built only once.
- `run()` takes what `execute()` takes and returns a `RunResult`: the `result`, the `debugData` of the nodes a debug node is connected to and the time every node took in ms, `nodeTimings` for all nodes and `runTimings` for those that ran and were not read from the cache. `outputBytes` are the bytes of the outputs of the nodes that ran, the nodes inside a group under the ids `expandGroups()` gives them.
- When a node fails, `execute()` throws a `NodeExecutionError`, its `info` names the node, the inputs it was missing and where a Rust panic happened. A runtime with `validate: true` throws a `GraphValidationError` before anything runs, see [Check a graph](#check-a-graph).
- `MemoryRuntimeExecutor`, `MemoryRuntimeCache` and `WasmNodeRegistry` are the building blocks of `createRuntime()`. Subclass `WasmNodeRegistry` to load the nodes from somewhere else.
- `migrateGraph()` and `CURRENT_GRAPH_VERSION` migrate graphs that don't come from a project file.

## Publishing

`.gitea/workflows/release.yaml` publishes the package: a tag `vX.Y.Z` publishes `X.Y.Z` as `latest`, a push to `main` runs `.gitea/scripts/publish-dev.sh` once the tests pass and publishes a dev version. `DRY_RUN=1 .gitea/scripts/publish-dev.sh` packs the dev version without publishing it.

The registry keeps every version it was given, also the dev versions. A cleanup rule of the owner removes the old ones (Settings → Packages → Cleanup rules on git.max-richter.dev): type npm, keep the versions matching `^\d+\.\d+\.\d+$`, remove the versions matching `-dev\.` that are older than 30 days. A single version is removed with `DELETE /api/v1/packages/max/npm/@nodarium%2Fruntime/<version>`.

## License

MIT, see [LICENSE](./LICENSE). This covers the runtime, the compiled nodes and the templates. The substrates of the `max/myzel/mesh` node are baked from CC0 scans of [Poly Haven](https://polyhaven.com).
