# window.nodarium

The app puts one object on `window` for a tool or an agent that drives it in a browser: load a graph, build it, read what it gave and take a picture of it. It is there in the dev server and on the deployed site. It has the power a visitor has with the mouse and no more: it works on the open graph, `graph.load` and `graph.loadTemplate` open a new project and never overwrite one, and it doesn't touch the settings of the app or other projects.

The types are in `app/src/lib/agent-api/types.ts`, the API itself in `app/src/lib/agent-api/agent-api.ts`. `version` is `1`; a change that breaks a caller raises it.

## A session

```js
await nodarium.ready;
const result = await nodarium.graph.load(myGraph, { title: 'agent test' });
// { ok: true, issues: [], vertices: 4228, faces: 8064,
//   bounds: { min: [-0.2, -0.03, -0.37], max: [0.52, 0.55, 0.37] },
//   ms: { total: 2, nodes: { 1: 0, 2: 0, 3: 0 } } }
```

`window.nodarium` exists once the app has started, which is after the `load` event of the page. With Playwright:

```ts
await page.goto('http://localhost:4173/?tutorial=0');
await page.waitForFunction(() => window.nodarium, null, { polling: 100 });
const result = await page.evaluate((graph) => window.nodarium!.graph.load(graph), myGraph);
```

`polling: 100` matters in a page that gets no animation frames, Playwright polls with `requestAnimationFrame` otherwise. `?tutorial=0` keeps Planty closed for the visit, without remembering it. `app/e2e/agent-api.test.ts` uses every method.

## The object

| member                                      | what it does                                                                                                         |
| ------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `version`, `app`                            | `1`, and the version and commit of the app                                                                           |
| `ready`                                     | resolves once the open project is built and the definitions of every node are loaded                                 |
| `idle()`                                    | resolves once nothing is being built or waits to be, see below                                                       |
| `graph.get()`                               | a copy of the open graph as it is saved                                                                              |
| `graph.load(graph, { title })`              | opens a graph or a project file as a new project, as File → Import does, and resolves with its first build           |
| `graph.loadTemplate(id)`                    | opens a template as a new project, as the project browser does                                                       |
| `graph.addNode(type, props)`                | adds a node in the middle of the editor, returns its id                                                              |
| `graph.removeNode(id)`                      | removes a node and its edges                                                                                         |
| `graph.connect(from, output, to, input)`    | connects an output to an input, an edge already in the input is replaced                                             |
| `graph.disconnect(to, input)`               | removes the edge into an input                                                                                       |
| `graph.setProp(id, key, value)`             | sets an input of a node, the value as the graph file has it (the index of a select)                                  |
| `graph.setSettings(settings)`               | sets settings of the graph, like `seed` or `resolution.circle`                                                       |
| `graph.format()`                            | lays the nodes out in columns, as "Format nodes" does                                                                |
| `graph.undo()`, `graph.redo()`              | as in the editor                                                                                                     |
| `nodes.list()`, `nodes.get(type)`           | the definitions of the nodes, with their inputs; all of them once `ready` has resolved                               |
| `run({ seed })`                             | builds the open graph, with `seed` as its seed if given, and resolves with the result                                |
| `lastResult()`                              | the result of the last build                                                                                         |
| `benchmark({ runs = 20, warmups = 3 })`     | builds the graph with random seeds and gives the times in ms, `{ samples, median, stdev }`; the graph keeps its seed |
| `viewer.screenshot({ size = 1024, views })` | a PNG of the last result                                                                                             |

Every change through `graph.*` goes through the editor and is one undo step. The methods that change the graph return at once, the build follows; `await nodarium.idle()` and then `lastResult()` read it. A mistake, like a node or input that doesn't exist or an edge between sockets of different types, throws an `Error` that says what is wrong.

A result:

```ts
type BuildResult = {
  ok: boolean; // the build didn't fail and the graph has no issue of the severity "error"
  issues: GraphIssue[]; // what validateGraph() finds, those of the file for graph.load
  error?: RuntimeErrorInfo; // the node that failed, as the error log has it
  vertices: number;
  faces: number;
  bounds?: { min: Vec3; max: Vec3 }; // in meters, missing when nothing is shown
  ms: { total: number; nodes: Record<number, number> };
};
```

## idle

`idle()` reads the state of the page, not the time: it resolves when no project is being read from the database or loaded by the editor, the editor has no build it asked for and not yet sent (a change waits up to 10 ms, a loaded graph 100 ms), no change is held back by a drag in the viewer, and the page has no build running or waiting for the one that runs. Two changes in quick succession are built one after the other, or the second one alone, and `idle()` waits for the last.

## A picture

`viewer.screenshot({ size, views })` renders the result of the last build in a worker, without the viewer and without animation frames, so it works in a window that is not visible. `views` takes `default` (the angle of the viewer), `front`, `side` and `top`, the image has them next to each other, each `size` pixels square, fitted to what it shows, on a transparent background. It shows the result, not the camera of the user: the same graph gives the same picture.

```ts
const base64 = await page.evaluate(async () => {
  const blob = await window.nodarium!.viewer.screenshot({ views: ['front', 'side', 'top'] });
  const bytes = new Uint8Array(await blob.arrayBuffer());
  return btoa(Array.from(bytes, (b) => String.fromCharCode(b)).join(''));
});
fs.writeFileSync('plant.png', Buffer.from(base64, 'base64'));
```

The viewer itself also draws once without a frame, 500 ms after a result arrived and no frame came.

## Without the API

The viewer has a line of status with the vertices, the faces and the time of the last build, in an element with `role="status"`. The input File → Import uses is labelled "Import a project file", a tool can give it a file without the dialog of the menu.
