# Nodarium — LLM Reference

## What It Is

Nodarium is a **node-based visual programming editor**. Users wire together nodes on a 2D canvas; each node is a WebAssembly module that receives typed inputs and produces typed outputs. A live Three.js viewer renders the resulting 3D geometry/paths/instances.

---

## Repository Layout

```
/
├── app/                        # SvelteKit web app
│   └── src/
│       ├── routes/+page.svelte # App entry point
│       └── lib/
│           ├── graph-interface/  # Canvas editor (UI + state)
│           ├── runtime/          # WASM execution engine
│           ├── node-registry/    # Fetch & cache node definitions
│           ├── project-manager/  # IndexDB persistence
│           ├── result-viewer/    # Three.js 3D output
│           ├── sidebar/          # UI panels
│           └── settings/         # App + graph settings
├── packages/
│   ├── types/    # Shared TypeScript types + Zod schemas
│   ├── utils/    # Logging, hashing, WASM wrapping, perf
│   ├── ui/       # Reusable Svelte UI components
│   ├── planty/   # Tutorial system
│   └── macros/   # Build-time macros
└── docs/
```

---

## Core Architecture

```
User Interaction
  └── GraphInterface
        ├── GraphState   ← UI: selection, camera, mouse, clipboard
        └── GraphManager ← Logic: nodes, edges, history, serialization
              ├── NodeRegistry   ← fetches WASM definitions (remote API + IndexDB cache)
              ├── HistoryManager ← undo/redo (jsondiffpatch deltas)
              └── emit('result') → RuntimeExecutor
                                      └── node.execute(Int32Array) per node
                                            └── ResultViewer (Three.js/Threlte)
```

**Event flow:**

1. User edits graph → GraphManager mutates state
2. GraphManager emits `save` → ProjectManager persists to IndexDB
3. GraphManager emits `result` → Runtime executes graph → Viewer updates

---

## Critical Files

| File                                                   | Role                                                                  |
| ------------------------------------------------------ | --------------------------------------------------------------------- |
| `app/src/routes/+page.svelte`                          | Wires all systems; creates GraphManager, runtime, registry            |
| `app/src/lib/graph-interface/graph-manager.svelte.ts`  | Central graph logic: createNode, createEdge, serialize, load, history |
| `app/src/lib/graph-interface/graph-state.svelte.ts`    | UI state: camera, selection, mouse, clipboard, groupSelectedNodes     |
| `app/src/lib/graph-interface/graph/Graph.svelte`       | Canvas renderer                                                       |
| `app/src/lib/graph-interface/node/Node.svelte`         | 3D mesh node (Three.js shader)                                        |
| `app/src/lib/graph-interface/node/NodeHTML.svelte`     | HTML overlay: labels + parameters                                     |
| `app/src/lib/graph-interface/node/NodeHeader.svelte`   | Node title bar                                                        |
| `app/src/lib/graph-interface/keymaps.ts`               | Keyboard shortcuts                                                    |
| `app/src/lib/graph-interface/helpers/nodeHelpers.ts`   | Node height calculations                                              |
| `app/src/lib/graph-interface/graph/colors.svelte.ts`   | Socket type → color mapping                                           |
| `app/src/lib/runtime/runtime-executor.ts`              | Executes nodes in DAG order; expandGroups()                           |
| `app/src/lib/node-registry/index.ts`                   | RemoteNodeRegistry entry                                              |
| `app/src/lib/node-registry/groupNode.ts`               | Built-in group node definition                                        |
| `app/src/lib/node-registry/debugNode.ts`               | Built-in debug node                                                   |
| `app/src/lib/sidebar/panels/ActiveNodeSettings.svelte` | Per-node settings panel                                               |
| `packages/types/src/types.ts`                          | Graph, NodeInstance, NodeDefinition, Edge, GroupDefinition            |
| `packages/types/src/inputs.ts`                         | NodeInput union types (float, vec3, geometry, path, …)                |
| `packages/utils/src/wasm.ts`                           | createWasmWrapper() — wraps WASM bytes into a NodeDefinition          |

---

## Key Types

```typescript
// packages/types/src/types.ts

type NodeId = `${string}/${string}/${string}`; // e.g. "max/plantarium/stem"

type NodeInstance = {
  id: number;
  type: NodeId;
  position: [number, number];
  props?: Record<string, number | number[]>; // current parameter values
  meta?: { title?: string; lastModified?: string };
  state: NodeRuntimeState; // runtime-only, NOT serialized
};

type NodeRuntimeState = {
  type?: NodeDefinition; // resolved definition
  parents?: NodeInstance[];
  children?: NodeInstance[];
  x?: number;
  y?: number; // interpolated position
  mesh?: Mesh; // Three.js mesh reference
  ref?: HTMLElement;
};

type NodeDefinition = {
  id: NodeId;
  inputs?: Record<string, NodeInput>;
  outputs?: string[]; // output type names
  meta?: { title?: string; description?: string };
  execute(input: Int32Array): Int32Array; // WASM function
};

// Edge: [fromNode, outputIndex, toNode, inputSocketName]
type Edge = [NodeInstance, number, NodeInstance, string];

type Graph = {
  nodes: NodeInstance[];
  edges: [number, number, number, string][]; // serialized (IDs, not refs)
  settings: Record<string, unknown>;
  groups: GroupDefinition[];
};

type GroupDefinition = {
  id: number;
  nodes: NodeInstance[];
  edges: Edge[];
  inputs?: Record<string, NodeInput>;
  outputs?: string[];
};
```

### NodeInput socket types

`float` | `integer` | `boolean` | `select` | `seed` | `vec3` | `geometry` | `path` | `shape` | `color` | `*` (wildcard)

Each input can have: `value` (default), `label`, `hidden`, `external`, `setting` (link to graph setting), `accepts` (extra compatible types).

---

## Patterns & Conventions

### Svelte 5 reactivity

The codebase uses Svelte 5 runes throughout — `$state`, `$derived`, `$effect`. Collections use `SvelteMap<K,V>` and `SvelteSet<T>` (from `svelte/reactivity`) instead of plain Map/Set so that mutations trigger reactive updates.

### Context API

`GraphManager` and `GraphState` are provided via Svelte context (`setContext` / `getContext`) inside `GraphInterface`. All child components (Node, Edge, etc.) consume them via context rather than props.

### Edge representation

In memory, edges are `[NodeInstance, outputIndex, NodeInstance, inputSocketName]` — direct object references for fast traversal. On serialization (`Graph.edges`), they become `[nodeId, outputIndex, nodeId, inputSocketName]` (plain IDs).

### Socket compatibility

```typescript
areSocketsCompatible(outputType: string, inputType: string | string[]): boolean
// '*' wildcard matches any type; 'geometry' accepts ['geometry', 'instances']
```

### WASM execution interface

Every node exposes a single function: `execute(input: Int32Array): Int32Array`.
Data encoding (Plantarium):

- `[0, stemDepth, ...x,y,z,thickness]` — path
- `[1, vertexCount, faceCount, ...faces, ...vertices, ...normals]` — geometry
- `[2, vertexCount, faceCount, instanceCount, stemDepth, ...]` — instances

### Event emitter

`GraphManager extends EventEmitter<{ save, result, settings }>`. Subscribe with `manager.on('result', cb)`. Used to decouple the editor UI from runtime execution and persistence.

### History

Every mutation goes through `HistoryManager`. Call `this.history.save(this.serialize())` before mutations; undo/redo replays jsondiffpatch deltas.

### Internal node IDs

Built-in nodes use the `__internal/` namespace: `__internal/group/instance`, `__internal/node/debug`. Virtual boundary nodes use `__virtual/`: `__virtual/group/input`, `__virtual/group/output`.

---

## In-Progress: Node Groups (`feat/group-node-own`)

Group selected nodes with **Ctrl+G**. A `GroupDefinition` is stored in `Graph.groups[]`; a group instance node (`__internal/group/instance`) referencing it by `props.groupId` replaces the selected nodes.

**Known gaps as of 2026-05-03:**

| Issue                                                                                     | Location                                                           |
| ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| `createGroupNode()` called but not defined                                                | `graph-state.svelte.ts:334` → missing in `graph-manager.svelte.ts` |
| Runtime expects `group.graph.nodes/edges`; schema has flat `nodes/edges`                  | `runtime-executor.ts` vs `types.ts`                                |
| Runtime expects `group.inputs` as array; schema defines it as `Record<string, NodeInput>` | Same mismatch                                                      |
| `enterGroupNode()` is a stub — no group navigation                                        | `graph-state.svelte.ts`                                            |
| `serialize()` writes parent-graph edges into group instead of group-internal edges        | `graph-manager.svelte.ts`                                          |

---

## Dev Commands

Run from `app/`:

```bash
npm run dev          # start dev server (Vite)
npm run build        # production build
npm run check        # svelte-check + tsc
npm run lint         # eslint
npm run test         # unit (vitest) + e2e (playwright)
npm run test:unit    # vitest only
npm run test:e2e     # playwright only
npm run bench        # benchmark runner
```
