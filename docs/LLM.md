# Nodarium - LLM Documentation

## Overview

Nodarium is a **WebAssembly-based visual programming language** for creating procedural 3D plants. The app features a node-based interface where users connect WASM modules to generate plant models in real-time. Currently used to develop https://nodes.max-richter.dev, a procedural modelling tool for 3D plants.

## Architecture

### Core Components

#### 1. Node System (`app/static/nodes/`)

WASM-based nodes that perform computations. All nodes must implement the NodeDefinition interface.

- **Node Storage**: `app/static/nodes/max/plantarium/`
  - `box.wasm` - Box geometry node
  - `branch.wasm` - Branch generation
  - `float.wasm` - Float value node
  - `gravity.wasm` - Gravity/physics node
  - `instance.wasm` - Instance rendering
  - `leaf.wasm` - Leaf geometry
  - `math.wasm` - Math operations
  - `noise.wasm` - Noise generation
  - `output.wasm` - Output node
  - `random.wasm` - Random value generation
  - `rotate.wasm` - Rotation node
  - `shape.wasm` - Shape geometry
  - `stem.wasm` - Stem generation
  - `triangle.wasm` - Triangle geometry
  - `vec3.wasm` - Vector3 node

- **Node Registry**: `app/src/lib/node-registry.ts`
  - Loads and manages WASM nodes
  - `getNodeWasm()` - Creates WASM wrapper from bytes
  - `getNode()` - Retrieves node definition

- **Debug Node**: `app/src/lib/node-registry/debugNode.js`
  - Special debug node with wildcard inputs
  - Variable-height nodes and parameters
  - Quick-connect shortcut

#### 2. Graph Interface

Visual node editor built with Svelte 5.

- **Main Wrapper**: `app/src/lib/graph-interface/graph/Wrapper.svelte`
  - Entry point for graph interface
  - Manages GraphManager and GraphState

- **GraphManager**: `app/src/lib/graph-interface/graph-manager.svelte.ts`
  - Core entity managing the node graph
  - Handles node connections and execution flow

- **GraphState**: `app/src/lib/graph-interface/graph-state.svelte.ts`
  - Tracks UI state (selection, snapping, help, active nodes)

- **Graph Components**:
  - `app/src/lib/graph-interface/graph/` - Graph rendering
  - `app/src/lib/graph-interface/node/` - Node rendering
  - `app/src/lib/graph-interface/edges/` - Edge rendering
  - `app/src/lib/graph-interface/components/` - UI components (AddMenu, Socket, etc.)
  - `app/src/lib/graph-interface/debug/` - Debug overlays
  - `app/src/lib/graph-interface/background/` - Grid/dots backgrounds

- **Helpers**:
  - `app/src/lib/helpers/` - Utility functions
  - `app/src/lib/helpers/createKeyMap.ts` - Keyboard shortcuts

#### 3. Runtime Execution

Performs graph execution via WASM nodes.

- **Runtime Executors** (`app/src/lib/runtime/`):
  - **MemoryRuntime**: Direct WASM execution in main thread
  - **WorkerRuntime**: WebWorker-based execution for performance
  - Both implement the RuntimeExecutor interface

- **Runtime Cache**: `app/src/lib/runtime/cache.ts`
  - Memory-based caching for graph execution

- **Execution Flow**:
  1. Graph serialized from graph interface
  2. Runtime executes nodes in topological order
  3. Results passed through connected edges
  4. Final mesh output rendered

#### 4. 3D Viewer (`app/src/lib/result-viewer/`)

Three.js-based rendering for 3D output.

- **Viewer**: `app/src/lib/result-viewer/Viewer.svelte`
  - Renders generated 3D meshes
  - Uses @threlte/core (Svelte-Three.js wrapper)

#### 5. Application Structure (`app/src/routes/`)

SvelteKit application routing.

- **Main Page**: `app/src/routes/+page.svelte`
  - Combines GraphInterface + 3D Viewer
  - Manages runtime selection (memory vs worker)
  - Handles settings and performance tracking

- **Layout**: `app/src/routes/+layout.svelte`
  - Application shell

- **Server**: `app/src/routes/+layout.server.ts`
  - Loads git metadata and changelog

#### 6. Settings System (`app/src/lib/settings/`)

Application and graph settings.

- **App Settings**: `app/src/lib/settings/app-settings.svelte.ts`
  - Debug mode, themes, node interface options

- **NestedSettings**: `app/src/lib/settings/NestedSettings.svelte`
  - Recursive settings UI component

#### 7. Sidebar Panels (`app/src/lib/sidebar/`)

- `app/src/lib/sidebar/Sidebar.svelte` - Main sidebar
- `app/src/lib/sidebar/panels/` - Individual panels:
  - `ActiveNodeSettings.svelte` - Selected node properties
  - `BenchmarkPanel.svelte` - Performance benchmarking
  - `Changelog.svelte` - Version history
  - `ExportSettings.svelte` - Export options
  - `GraphSource.svelte` - Graph JSON view
  - `Keymap.svelte` - Keyboard shortcuts

#### 8. Project Management (`app/src/lib/project-manager/`)

- `app/src/lib/project-manager/project-manager.svelte` - Project save/load
- Uses IndexedDB for persistence

#### 9. Node Store (`app/src/lib/node-store/`)

- `app/src/lib/node-store/NodeStore.svelte`
- Remote node registry management
- IndexDBCache for offline storage

#### 10. Graph Templates (`app/src/lib/graph-templates/`)

Pre-built graph templates for testing:

- Grid, Tree, LottaFaces, LottaNodes, LottaNodesAndFaces

## Key Types (`app/src/lib/types.ts`)

```typescript
interface NodeDefinition {
  id: string;
  name: string;
  inputs: Socket[];
  outputs: Socket[];
  parameters: Parameter[];
  execute: (inputs: any[], parameters: any[]) => any[];
}

interface Socket {
  id: string;
  name: string;
  type: string; // datatype (e.g., "number", "vec3", "*")
  defaultValue?: any;
  optional?: boolean;
}

interface Parameter {
  id: string;
  name: string;
  type: string;
  defaultValue: any;
  min?: number;
  max?: number;
  options?: string[];
}

interface Graph {
  nodes: NodeInstance[];
  edges: Edge[];
}

interface NodeInstance {
  id: number;
  nodeId: string;
  position: { x: number; y: number };
  parameters: Record<string, any>;
}

interface Edge {
  id: number;
  fromNode: number;
  fromSocket: string;
  toNode: number;
  toSocket: string;
}
```

## Development Workflow

### Prerequisites

- Node.js
- pnpm
- Rust
- wasm-pack

### Build Commands

```bash
# Install dependencies
pnpm i

# Build WASM nodes
pnpm build:nodes

# Start development server
cd app && pnpm dev

# Run tests
cd app && pnpm test

# Lint and typecheck
cd app && pnpm lint
cd app && pnpm check

# Format code
cd app && pnpm format
```

### Creating New Nodes

See `docs/DEVELOPING_NODES.md` for detailed instructions on creating custom WASM nodes.

## Features

### Current Features

- Visual node-based programming with real-time 3D preview
- WebAssembly nodes for high-performance computation
- Debug node with wildcard inputs and runtime integration
- Color-coded node sockets and edges (indicating data types)
- Variable-height nodes and parameters
- Edge dragging with valid socket highlighting
- InputNumber snapping to predefined values (Alt+click)
- Project save/load with IndexedDB
- Performance monitoring and benchmarking
- Changelog viewer
- Advanced mode settings

### UI Components

- **InputNumber**: Numeric input with arrow controls
- **InputColor**: Color picker
- **InputShape**: Shape selector with preview
- **InputSelect**: Dropdown with options

## File Structure

```
nodarium/
├── app/
│   ├── src/
│   │   ├── lib/
│   │   │   ├── config.ts
│   │   │   ├── graph-interface/    # Node editor
│   │   │   ├── graph-manager.svelte.ts
│   │   │   ├── graph-state.svelte.ts
│   │   │   ├── graph-templates/   # Test templates
│   │   │   ├── grid/
│   │   │   ├── helpers/
│   │   │   ├── node-registry.ts
│   │   │   ├── node-registry/     # Node loading
│   │   │   ├── node-store/
│   │   │   ├── performance/
│   │   │   ├── project-manager/
│   │   │   ├── result-viewer/       # 3D viewer
│   │   │   ├── runtime/           # Execution
│   │   │   ├── settings/         # App settings
│   │   │   ├── sidebar/
│   │   │   └── types.ts
│   │   └── routes/
│   │       ├── +page.svelte
│   │       └── +layout.svelte
│   ├── static/
│   │   └── nodes/
│   │       └── max/
│   │           └── plantarium/    # WASM nodes
│   └── package.json
├── docs/
│   ├── ARCHITECTURE.md
│   ├── DEVELOPING_NODES.md
│   ├── NODE_DEFINITION.md
│   └── PLANTARIUM.md
├── nodes/                        # WASM node source (Rust)
└── package.json
```

## Release Process

1. Create annotated tag:
   ```bash
   git tag -a v1.0.0 -m "Release notes"
   git push origin v1.0.0
   ```

2. CI workflow:
   - Runs lint, format check, type check
   - Builds project
   - Updates package.json versions
   - Generates CHANGELOG.md
   - Creates Gitea release
