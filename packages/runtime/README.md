# @nodarium/runtime

Runs a [Nodarium](https://nodes.max-richter.dev) graph without the app and turns its result into vertices and faces. The package brings the compiled nodes (Plantarium and Myzel) with it, so it generates the same plants and mushrooms as the app of the same version. It needs no three.js, the result is plain typed arrays.

Templates are not part of the package: build or pick a graph in the app, export it and pass the file in.

## Install

The package is in the npm registry of git.max-richter.dev. Add the scope to the `.npmrc` of your project:

```
@nodarium:registry=https://git.max-richter.dev/api/packages/max/npm/
```

```sh
npm install @nodarium/runtime
```

## Export a project from the app

Open the plant or mushroom in the app and choose **File → Export .json**. The app saves it as `<title>.nodarium.json`. `parseProjectFile()` reads these files, and the bare graphs older versions of the app saved. It checks them and migrates them to the format of the runtime. A graph made with a newer version of the app than the package throws a `GraphVersionError`.

## Node

`@nodarium/runtime/node` reads the nodes from the package, `createRuntime()` needs no arguments:

```ts
import { createRuntime, decodeResult, parseProjectFile } from '@nodarium/runtime/node';
import { readFile } from 'node:fs/promises';

const { graph } = parseProjectFile(JSON.parse(await readFile('fern.nodarium.json', 'utf8')));

const runtime = createRuntime();
const result = await runtime.execute(graph, { ...graph.settings, seed: 7 });
const { geometries, instances, meshes, vertexCount, faceCount } = decodeResult(result);
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
const { meshes } = decodeResult(await runtime.execute(graph, graph.settings));
```

The registry fetches `<baseUrl>/<user>/<collection>/<node>.wasm`, like `/nodes/max/myzel/output.wasm`. In a SvelteKit or Vite project, copy the folder into `static` (or `public`) once it is installed, for example with a `postinstall` script:

```json
{
  "scripts": {
    "postinstall": "rm -rf static/nodes && cp -R node_modules/@nodarium/runtime/nodes static/nodes"
  }
}
```

Copy them again after every update of the package, the nodes have to match the runtime.

A mushroom takes seconds to build, plants are much faster. To keep the page responsive, run the runtime in a Web Worker and transfer `result.buffer` back to the page. The nodes are compiled synchronously when they are registered, the largest is below 200 KB, the app compiles them on the main thread as well.

On the server (SvelteKit `+page.server.ts`, an API route), use `@nodarium/runtime/node`. It finds the nodes through `import.meta.url`, so leave the package external in the SSR build (the Vite default) or pass the folder to `FileNodeRegistry` yourself.

## The result

`execute()` returns one `Int32Array`. `decodeResult()` splits it into typed arrays. They are views into the result, copy them before you change or transfer it.

```ts
type DecodedResult = {
  geometries: GeometryData[]; // Plantarium geometry, one per path
  instances: InstancesData[]; // Plantarium instances like leaves
  meshes: MeshData[]; // Myzel meshes
  vertexCount: number; // of everything, every instance counted
  faceCount: number;
};

type MeshData = {
  vertexCount: number;
  faceCount: number;
  faces: Int32Array; // three vertex indices per face
  positions: Float32Array; // x, y, z per vertex
  normals: Float32Array; // x, y, z per vertex
  colors?: Uint8Array; // Myzel only: RGBA8 per vertex in sRGB, alpha 0 where a vertex has no color
  groups?: Int32Array; // Myzel only: the material group (tissue) of every vertex
};

type GeometryData = MeshData & { pathId: number }; // the path the geometry was made from

type InstancesData = MeshData & {
  instanceCount: number;
  depth: number;
  matrices: Float32Array; // a column-major 4x4 matrix per instance
  pathIds: Int32Array; // the path every instance sits on
};
```

Plants come out as `geometries` and `instances`, mushrooms as `meshes`. A Myzel mesh carries the colors of nodes like `set_color` and a group for every vertex, so a viewer can give every tissue its own material. Vertices without a color of their own have alpha 0 and take the color of the material.

With three.js, a mesh becomes a `BufferGeometry`:

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

- `cache: true` in `createRuntime()` keeps the results of the nodes, so a run with a changed setting or seed only runs the nodes the change reaches.
- When a node fails, `execute()` throws a `NodeExecutionError`, its `info` names the node, the inputs it was missing and where a Rust panic happened.
- `MemoryRuntimeExecutor`, `MemoryRuntimeCache` and `WasmNodeRegistry` are the building blocks of `createRuntime()`. Subclass `WasmNodeRegistry` to load the nodes from somewhere else.
- `migrateGraph()` and `CURRENT_GRAPH_VERSION` migrate graphs that don't come from a project file.
