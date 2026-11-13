# @nodarium/runtime

Runs a [Nodarium](https://nodes.max-richter.dev) graph without the app and turns its result into vertices and faces. The package brings the compiled nodes (Plantarium and Myzel) with it, so it generates the same plants and mushrooms as the app of the same version. It needs no three.js, the result is plain typed arrays.

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
const { meshes } = decodeResult(result);
```

```ts
import { loadTemplate, templates } from '@nodarium/runtime/templates';

const mushrooms = templates.filter((t) => t.category === 'Mushrooms');
const graph = await mushrooms[0].load(); // or await loadTemplate('porcini')
```

Every template is a `Graph` of the current graph version, `parseProjectFile()` isn't needed. Its `settings` are the settings of the template in the app: resolutions like `resolution.curve` or `resolution.field`, the `time.t` of most mushrooms and, in a few, a seed. Pass them to `execute()` and put your own seed or settings on top, as above. Settings that are missing take the default of their node. The graphs are shared by everyone who imports them, `structuredClone()` one before you change it.

`unit` is the unit the app shows lengths in. The graph stores meters either way, a unit of `cm` only means that the template is small.

| id                   | name              | category  | unit |
| -------------------- | ----------------- | --------- | ---- |
| `default-plant`      | Default Plant     | Plants    | m    |
| `fern`               | Fern              | Plants    | cm   |
| `willow`             | Weeping Willow    | Plants    | m    |
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
| `foxglove`           | Foxglove          | Plants    | m    |
| `devils-backbone`    | Devil's Backbone  | Plants    | cm   |
| `knotweed`           | Japanese Knotweed | Plants    | m    |
| `wild-carrot`        | Wild Carrot       | Plants    | cm   |
| `champignon-groomed` | Champignon        | Mushrooms | cm   |
| `chestnut`           | Chestnut mushroom | Mushrooms | cm   |
| `fly-agaric`         | Fly agaric        | Mushrooms | cm   |
| `porcini`            | Porcini           | Mushrooms | cm   |
| `shiitake`           | Shiitake          | Mushrooms | cm   |
| `shaggy-inkcap`      | Shaggy inkcap     | Mushrooms | cm   |
| `chanterelle`        | Chanterelle       | Mushrooms | cm   |
| `oyster`             | Oyster mushroom   | Mushrooms | cm   |
| `honey-fungus`       | Honey fungus      | Mushrooms | cm   |
| `sulphur-tuft`       | Sulphur tuft      | Mushrooms | cm   |
| `false-chanterelle`  | False chanterelle | Mushrooms | cm   |
| `bitter-bolete`      | Bitter bolete     | Mushrooms | cm   |

The ids stay, a template that is improved keeps its id. To add a template in this repository, put its graph into `packages/runtime/templates/<id>.json`, add a line `plant(...)` or `mushroom(...)` to the list in `templates/index.ts` and a row to the table above. The app lists it in its project browser from there.

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

## Publishing

`.gitea/workflows/release.yaml` publishes the package: a tag `vX.Y.Z` publishes `X.Y.Z` as `latest`, a push to `main` runs `.gitea/scripts/publish-dev.sh` once the tests pass and publishes a dev version. `DRY_RUN=1 .gitea/scripts/publish-dev.sh` packs the dev version without publishing it.

The registry keeps every version it was given, also the dev versions. A cleanup rule of the owner removes the old ones (Settings → Packages → Cleanup rules on git.max-richter.dev): type npm, keep the versions matching `^\d+\.\d+\.\d+$`, remove the versions matching `-dev\.` that are older than 30 days. A single version is removed with `DELETE /api/v1/packages/max/npm/@nodarium%2Fruntime/<version>`.

## License

MIT, see [LICENSE](./LICENSE). This covers the runtime, the compiled nodes and the templates. The substrates of the `max/myzel/mesh` node are baked from CC0 scans of [Poly Haven](https://polyhaven.com).
