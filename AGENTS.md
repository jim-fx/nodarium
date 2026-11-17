# AGENTS.md

Nodarium is a node editor for procedural 3D models that runs in the browser. Every node is a WebAssembly module, written in Rust; a TypeScript runtime runs the graph of nodes, and a SvelteKit app edits the graph and shows the result with Three.js. Two node systems are built on it: Plantarium (plants) and Myzel (fungi).

## Map

| path                                        | what                                                                                                                                                                  |
| ------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `app/`                                      | the app: node editor in `app/src/lib/graph-interface`, viewer in `app/src/lib/result-viewer`, tools in `app/scripts`, bench in `app/benchmark`, e2e test in `app/e2e` |
| `packages/runtime/`                         | runs graphs, migrates old ones, holds the templates; published as `@nodarium/runtime`                                                                                 |
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

`pnpm qa` runs `pnpm lint`, `pnpm format:check`, `pnpm check` and `pnpm test:unit`, about 25 s in all. The unit tests alone take about 1 s for the runtime and 6 s for the app. CI (`.gitea/workflows/release.yaml`, on every push and pull request) runs the same steps, then builds the app with `pnpm build` and runs the Playwright test (`pnpm test:e2e`). On every push, `.gitea/workflows/benchmark.yaml` also runs `pnpm -C app bench` (about 45 s on a desktop) and stores the numbers in the repository `max/nodarium-benchmarks`.

To look at a graph without a browser: `pnpm -C app render-graph fern` writes front, side and top views to `screenshots/` in `app/` (templates by name, `templates` for all, or the path of a graph or project file). `pnpm -C app format-graph` lays out the nodes of a template. To measure: `pnpm -C app bench` for whole graphs, `pnpm -C app bench:paths` for the path nodes. Each script explains its flags at the top of its file in `app/scripts`.

## Rules

- Run `pnpm dprint fmt` before every commit. Never run `cargo fmt`: the repository is formatted by dprint, and rustfmt rewrites about 25 files.
- Node wasm links with `--allow-undefined` (`.cargo/config.toml`): since rustc 1.96 the host functions a node imports would be link errors otherwise. Keep it when touching the Cargo config.
- Components in `packages/ui` use Tailwind classes, not `<style>` blocks.
- `app/src/lib/graph-interface` stays self-contained: no new imports from other `#lib` or `$lib` modules into it, it is meant to become its own package.
- Comments only for reasons the code can't show. No comments that restate the code.
- Nodes have few inputs. Variation and curves come from shaping an input with the random, curve and alpha nodes, not from extra inputs on every node. A generic node comes before a custom one.
- A link carries exactly its type. No hidden values inside a list of another type (for example a hinge inside a path list).
- A browser window that is not visible gets no animation frames, so the viewer stays black and looks broken. To check the app in a browser, use a Playwright script as `app/e2e/main.test.ts` does, or `render-graph` above.

## Where the knowledge is

- `docs/ARCHITECTURE.md`: nodes, the runtime, migrations, how the app is put together, the key types
- `docs/ABI.md`: how the runtime and a node's wasm talk to each other
- `docs/NODE_DEFINITION.md` and `docs/DEVELOPING_NODES.md`: writing a node
- `docs/PLANTARIUM.md` and `docs/PLANT_TEMPLATES.md`: the data of the plant nodes, and how the plant templates are modelled
- `docs/MYZEL.md`: the fungi nodes and their workflow
- `docs/UX.md`: general UX practices for web apps
- `packages/runtime/README.md`: using the runtime without the app
- `app/README.md`: the template previews

`app/src/docs.test.ts` checks that every path and `pnpm` script named in this file and in `docs/` exists. Update it here when you move a file.
