# AGENTS.md

Nodarium is a node editor for procedural 3D models that runs in the browser. Every node is a WebAssembly module, written in Rust; a TypeScript runtime runs the graph of nodes, and a SvelteKit app edits the graph and shows the result with Three.js. Two node systems are built on it: Plantarium (plants) and Myzel (fungi).

## Map

| path                                        | what                                                                                                                                                                  |
| ------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `app/`                                      | the app: node editor in `app/src/lib/graph-interface`, viewer in `app/src/lib/result-viewer`, tools in `app/scripts`, bench in `app/benchmark`, e2e test in `app/e2e` |
| `packages/runtime/`                         | runs graphs, migrates old ones, holds the templates and the `nodarium` command; published as `@nodarium/runtime`                                                      |
| `packages/types/`, `packages/utils/`        | shared TypeScript types and helpers, and the Rust crates `nodarium_types` and `nodarium_utils`                                                                        |
| `packages/macros/`                          | the Rust macros every node uses (`nodarium_definition_file!`, `#[nodarium_execute]`)                                                                                  |
| `packages/myzel/`                           | `myzel_core`, the mesh and path formats of the Myzel nodes                                                                                                            |
| `packages/ui/`, `packages/planty/`          | the Svelte components of the app, and the tutorial system                                                                                                             |
| `nodes/max/plantarium/`, `nodes/max/myzel/` | one Rust crate per node                                                                                                                                               |

## Setup

```bash
pnpm bootstrap                 # pnpm install, builds ui and planty, builds the nodes
pnpm bootstrap --reuse-nodes   # in a worktree: copies app/static/nodes from the main checkout
```

`--reuse-nodes` only copies when nothing under `nodes/`, the Rust packages, `Cargo.toml` or `Cargo.lock` differs from the commit the main checkout is on; otherwise it builds. On a desktop with a warm pnpm store it takes about 10 s with `--reuse-nodes` and 20 s without. Without the ui and planty builds, `pnpm check` fails with "$app/tsconfig not found". After changing a node, run `pnpm build:nodes` again. Start the app with `pnpm -C app dev`.

## The loop

`pnpm qa` runs `pnpm lint`, `pnpm format:check`, `pnpm check` and `pnpm test:unit`, about 25 s in all. The unit tests alone take about 1 s for the runtime, 6 s for the app and 7 s for `packages/ui`, whose component tests run in a headless Firefox (once: `pnpm -C packages/ui exec playwright install firefox`). CI (`.gitea/workflows/release.yaml`, on every push and pull request) runs the same steps, then builds the app with `pnpm build` and runs the Playwright test (`pnpm test:e2e`). On every push, `.gitea/workflows/benchmark.yaml` also runs `pnpm -C app bench` (about 45 s on a desktop) and stores the numbers in the repository `max/nodarium-benchmarks`.

To look at a graph without a browser, use `pnpm nodarium` in the root (`packages/runtime/src/cli.ts`, the `nodarium` command of `@nodarium/runtime`). It takes a template id, the path of a graph or project file, or `-` for stdin, prints JSON with `--json` and exits with 1 on an error:

```bash
pnpm nodarium nodes                  # every node: its signature and the first sentence of its description
pnpm nodarium nodes stem noise       # the inputs of these nodes, with their descriptions
pnpm nodarium check plant.json       # every mistake validateGraph finds, with the fix
pnpm nodarium run fern --timings     # size and bounds, and the ms and output bytes of every node
pnpm nodarium render fern            # front, side and top view in screenshots/fern.png
pnpm nodarium render fern --node 14  # the same of what comes out of node 14
pnpm nodarium link plant.json        # a link that opens the graph in the app
```

`--set 4.length=0.3` changes an input and `--set resolution.curve=8` a setting for one run, `pnpm -s nodarium run fern --print-graph` prints a template to start a graph from. [docs/GRAPHS.md](docs/GRAPHS.md) explains the file. `pnpm -C app render-graph fern` is `render` that also knows the test graphs of the app (`templates` renders all of them), `pnpm -C app format-graph` lays out the nodes of a template like "Format nodes" of the editor. Each script explains its flags at the top of its file in `app/scripts`.

To measure: `pnpm -C app bench` runs whole graphs (the six of CI, or the templates and files you name, `--quick` for a tenth of the runs) and ends with the median, p95 and spread of each. `--save <name>` keeps the run in `app/benchmark/saved/<name>`, `--compare <name>` shows the change of every number and exits with 1 when something got slower by more than 10 %. Both run every benchmark in 3 fresh processes, one after the other (about 2.5 min for the six, 3 times the plain bench), because the median moves more between processes than between the runs of one: a change counts only when the medians lie further apart than twice the spreads between the processes of both sides added. With one process on a side (`--quick`, a folder of CI), or when other work used more than 60 % of the machine, it can't tell noise from a change of a few percent and only counts one beyond twice or half. `pnpm -C app run profile porcini` lists every node with its ms, the bytes it got and gave and its wasm memory (`run`, because `pnpm profile` is a command of pnpm), `--dump-args dir` writes the inputs of every node for `pnpm -C app profile:node strands <args file> --cpu-prof`. `pnpm -C app bench:paths` measures the path nodes. All of them print their flags with `--help`.

## Rules

- Run `pnpm dprint fmt` before every commit. Never run `cargo fmt`: the repository is formatted by dprint, and rustfmt rewrites about 25 files.
- Node wasm links with `--allow-undefined` (`.cargo/config.toml`): since rustc 1.96 the host functions a node imports would be link errors otherwise. Keep it when touching the Cargo config.
- Components in `packages/ui` use Tailwind classes, not `<style>` blocks.
- `app/src/lib/graph-interface` stays self-contained: no new imports from other `#lib` or `$lib` modules into it, it is meant to become its own package.
- Comments only for reasons the code can't show. No comments that restate the code.
- Nodes have few inputs. Variation and curves come from shaping an input with the random, curve and alpha nodes, not from extra inputs on every node. A generic node comes before a custom one.
- A link carries exactly its type. No hidden values inside a list of another type (for example a hinge inside a path list).
- A browser window that is not visible gets no animation frames: the viewer draws once on its own, the graph stays black. To check the app in a browser, drive it with a Playwright script through `window.nodarium` ([docs/AGENT_API.md](docs/AGENT_API.md)): load a graph, wait for the build, read the result, take a picture with `viewer.screenshot()`. Open it with `?tutorial=0`, a template with `?template=fern` and a graph with the link of `nodarium link`. `nodarium render` above needs no browser.

## Where the knowledge is

- `docs/GRAPHS.md`: how to write a graph by hand
- `docs/ARCHITECTURE.md`: nodes, the runtime, migrations, how the app is put together, the key types
- `docs/ABI.md`: how the runtime and a node's wasm talk to each other
- `docs/NODE_DEFINITION.md` and `docs/DEVELOPING_NODES.md`: writing a node
- `docs/PLANTARIUM.md` and `docs/PLANT_TEMPLATES.md`: the data of the plant nodes, and how the plant templates are modelled
- `docs/MYZEL.md`: the fungi nodes and their workflow
- `docs/UX.md`: general UX practices for web apps
- `docs/AGENT_API.md`: `window.nodarium`, the API of the app for a tool or an agent in a browser
- `packages/runtime/README.md`: using the runtime without the app
- `app/README.md`: the template previews

`app/src/docs.test.ts` checks that every path and `pnpm` script named in this file and in `docs/` exists. Update it here when you move a file.
