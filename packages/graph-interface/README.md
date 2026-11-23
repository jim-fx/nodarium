# @nodarium/graph-interface

The node editor of [Nodarium](https://nodes.max-richter.dev) as a Svelte 5 component. It edits a `Graph` of `@nodarium/runtime` with the nodes of any `NodeRegistry`: the wasm nodes of the runtime, nodes written in JavaScript, or both. Running the graph is up to the app, the editor hands it the graph whenever it changed.

## Install

The packages are in the npm registry of git.max-richter.dev. Add the scope to the `.npmrc` of your project:

```
@nodarium:registry=https://git.max-richter.dev/api/packages/max/npm/
```

```sh
npm install @nodarium/graph-interface @nodarium/ui @nodarium/runtime @threlte/core @threlte/extras three
```

`@nodarium/runtime`, `@nodarium/ui`, `svelte`, `three` and Threlte are peer dependencies, the app has one copy of each. The three Nodarium packages are released together and share their version: install the same version of all three. `latest` is the last release, `dev` the last push to `main` (see the README of `@nodarium/runtime`).

The package ships `.svelte` files and imports its shaders with `?raw`, so it needs Vite with `@sveltejs/vite-plugin-svelte` (or SvelteKit).

## Styles

The editor and `@nodarium/ui` are styled with Tailwind 4. `@nodarium/ui/app.css` is a Tailwind entry of its own with the theme (`--color-layer-0` ... `--color-frame-violet`, the themes `theme-light`, `theme-solarized`, `theme-catppuccin`, `theme-high-contrast` as classes on `<html>`), the font and the classes of the ui components. Import it once, and add the editor to the sources of your own Tailwind entry, the path relative to that CSS file:

```css
/* src/app.css */
@import "tailwindcss";
@source "../node_modules/@nodarium/graph-interface/dist";
```

```svelte
<!-- src/routes/+layout.svelte -->
<script lang="ts">
  import '@nodarium/ui/app.css';
  import '../app.css';
</script>
```

## Mount the editor

```svelte
<script lang="ts">
  import GraphInterface, { type GraphOptions, readThemeColors } from '@nodarium/graph-interface';
  import { FetchNodeRegistry, type Graph, type NodeDefinition } from '@nodarium/runtime';
  import { Toast } from '@nodarium/ui';

  const circle: NodeDefinition = {
    id: 'me/vj/circle',
    meta: { title: 'Circle' },
    inputs: { radius: { type: 'float', value: 1, min: 0, max: 4 } },
    outputs: ['path'],
    execute: ([radius]) => makeCircle(radius)
  };

  // the wasm nodes of the runtime, its `nodes` folder served at /nodes, and the JS nodes
  const registry = new FetchNodeRegistry('/nodes', [circle, output]);
  registry.baseCollections = ['max/core', 'me/vj'];

  const options: GraphOptions = {
    backgroundType: 'grid',
    snapToGrid: true,
    showHelp: false,
    showDebugInfo: false,
    theme: 'dark',
    colors: readThemeColors(),
    nodeTimings: {}
  };

  let { graph }: { graph: Graph } = $props();
  let editor = $state<ReturnType<typeof GraphInterface>>();
</script>

{#key graph.id}
  <GraphInterface
    bind:this={editor}
    {graph}
    {registry}
    {options}
    onresult={(changed) => run(changed)}
    onsave={(changed) => store(changed)}
  />
{/key}
<Toast />
```

The editor fills its parent, give that a size. It loads `graph` once, when it mounts: key it on the graph to open another one. Its messages (a node type that is missing, a graph of a newer version) are toasts of `@nodarium/ui`, mount `<Toast />` to show them.

### Props

| prop                 | what                                                                                                                       |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `graph`              | the `Graph` to edit, migrated to the current version on load                                                               |
| `registry`           | a `NodeRegistry`, where the editor gets the definitions of the nodes                                                       |
| `options`            | `GraphOptions`, read through getters, so an object with getters makes them reactive                                        |
| `onresult`           | `(graph) => void`: the graph changed in a way that changes what it builds, run it                                          |
| `onsave`             | `(graph) => void`: the graph changed and the changes came to rest, store it                                                |
| `onregister`         | `(node, wasm) => void \| Promise<void>`: a `.wasm` file was dropped on the editor and `registry.register()` made it a node |
| `highlightedNodeIds` | nodes to outline apart from the selection                                                                                  |
| `onnodehover`        | `(nodeId \| undefined) => void`: the node under the pointer                                                                |

`GraphOptions`: `backgroundType` (`grid`, `dots`, `none`), `snapToGrid`, `showHelp` (the help on hover), `showDebugInfo` (node ids and timings), `theme` (a name, the colours are read again when it changes), `colors` (the `ThemeColors`, three.js colours the WebGL parts draw with; `readThemeColors()` reads them from the CSS variables of the theme, pass the same object again after the theme changed), and the optional `systemColors` (the colour of the bar on the header per collection, `{ 'me/vj': '#c65a19' }`), `safePadding` (room an overlay takes from the view), `nodeTimings` (ms per node id), `errorNodeId` (the node that made the last run fail), `helpSketch` (a component drawn in the help of a node) and `valuesOnly` (only the values of the inputs can change).

### The instance

`bind:this` gives the component with:

- `manager`, the `GraphManager`: `serialize()` returns the graph as it is now, `load(graph)` opens another graph, `createNode()`, `createEdge()`, `removeNode()`, `undo()`, `redo()`, `settings` and `setSetting()` for the settings the nodes of the graph declare (`collectSettings()` reads them from definitions), `status` (`loading`, `idle`, `error`). It is an event emitter of `save`, `result` and `usage`.
- `state`, the `GraphState` of the UI: selection, active node, camera. `state.centerNode()` frames the graph, the editor does it on its own only for nodes it placed. The camera of every graph id is kept in `localStorage`.
- `keymap`, the shortcuts of the editor, to list them in a help.

`NodePreview` draws a node of a definition without an editor, like in the node list of the app.

## The registry

`NodeRegistry` is an interface of `@nodarium/runtime`; the editor needs `load()`, `getNode()`, `getAllNodes()` and, for dropped wasm files, `register()`. `FetchNodeRegistry` and `WasmNodeRegistry` of the runtime take the JS nodes as their second and first argument. A registry of your own works the same, for example one that asks a `FetchNodeRegistry` for every node it doesn't know itself.

The add menu offers the nodes of the collections the graph uses and those of `baseCollections` (a collection is the first two parts of an id, `max/core` of `max/core/float`). Of those, it shows the nodes the registry has loaded: a registry with the optional `loadCollection(collection)` loads all of them, `listCollections()` adds collections the graph doesn't use yet. The JS nodes a `FetchNodeRegistry` is made with count as loaded from the start (passing them replaces its default list, add `debugNode` of the runtime to keep the debug node).

Nothing in the editor depends on `max/core/output`: any graph whose nodes the registry knows loads, also one that ends in an output node of the app (a type ending in `/output` is never muted).
