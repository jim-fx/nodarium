# Nodarium - AI Coding Agent Summary

## Project Overview

Nodarium is a WebAssembly-based visual programming language used to build <https://nodes.max-richter.dev>, a procedural 3D plant modeling tool. The system allows users to create visual node graphs where each node is a compiled WebAssembly module.

## Technology Stack

**Frontend (SvelteKit):**

- Framework: SvelteKit with Svelte 5
- 3D Rendering: Three.js via Threlte
- Styling: Tailwind CSS 4
- Build Tool: Vite
- State Management: Custom store-client package
- WASM Integration: vite-plugin-wasm, comlink

**Backend/Core (Rust/WASM):**

- Language: Rust
- Output: WebAssembly (wasm32-unknown-unknown target)
- Build Tool: cargo
- Procedural Macros: custom macros package

**Package Management:**

- Node packages: pnpm workspace (v10.28.1)
- Rust packages: Cargo workspace

## Directory Structure

```
nodarium/
├── app/                          # SvelteKit web application
│   ├── src/
│   │   ├── lib/                  # App-specific components and utilities
│   │   ├── routes/               # SvelteKit routes (pages)
│   │   ├── app.css               # Global styles
│   │   └── app.html              # HTML template
│   ├── static/
│   │   └── nodes/                # Compiled WASM node files served statically
│   ├── package.json              # App dependencies
│   ├── svelte.config.js          # SvelteKit configuration
│   ├── vite.config.ts            # Vite configuration
│   └── tsconfig.json             # TypeScript configuration
│
├── packages/                     # Shared workspace packages
│   ├── ui/                       # Svelte UI component library (published as @nodarium/ui)
│   │   ├── src/                  # UI components
│   │   ├── static/               # Static assets for UI
│   │   ├── dist/                 # Built output
│   │   └── package.json
│   ├── registry/                 # Node registry with IndexedDB persistence (@nodarium/registry)
│   │   └── src/
│   ├── types/                    # Shared TypeScript types (@nodarium/types)
│   │   └── src/
│   ├── utils/                    # Shared utilities (@nodarium/utils)
│   │   └── src/
│   └── macros/                   # Rust procedural macros for node development
│
├── nodes/                        # WebAssembly node packages (Rust)
│   └── max/plantarium/           # Plantarium nodes namespace
│       ├── box/                  # Box geometry node
│       ├── branch/               # Branch generation node
│       ├── float/                # Float value node
│       ├── gravity/              # Gravity simulation node
│       ├── instance/             # Geometry instancing node
│       ├── math/                 # Math operations node
│       ├── noise/                # Noise generation node
│       ├── output/               # Output node for results
│       ├── random/               # Random value node
│       ├── rotate/               # Rotation transformation node
│       ├── stem/                 # Stem geometry node
│       ├── triangle/             # Triangle geometry node
│       ├── vec3/                 # Vector3 manipulation node
│       └── .template/            # Node template for creating new nodes
│
├── docs/                         # Documentation
│   ├── ARCHITECTURE.md           # System architecture overview
│   ├── DEVELOPING_NODES.md       # Guide for creating new nodes
│   ├── NODE_DEFINITION.md        # Node definition schema
│   └── PLANTARIUM.md             # Plantarium-specific documentation
│
├── Cargo.toml                    # Rust workspace configuration
├── package.json                  # Root npm scripts
├── pnpm-workspace.yaml           # pnpm workspace configuration
├── pnpm-lock.yaml                # Locked dependency versions
└── README.md                     # Project readme
```

## Node System Architecture

### What is a Node?

Nodes are WebAssembly modules that:

- Have a unique ID (e.g., `max/plantarium/stem`)
- Define inputs with types and default values
- Define outputs they produce
- Execute logic when called with arguments

### Node Definition Schema

Nodes are defined via `definition.json` embedded in each WASM module:

```json
{
  "id": "namespace/category/node-name",
  "outputs": ["geometry"],
  "inputs": {
    "height": { "type": "float", "value": 1.0 },
    "radius": { "type": "float", "value": 0.1 }
  }
}
```

For now the outputs are limited to a single output.

### Node Execution

Nodes receive serialized arguments and return serialized outputs. The `nodarium_utils` Rust crate provides helpers for:

- Parsing input arguments
- Creating geometry data
- Concatenating output vectors

### Node Registration

Nodes are:

1. Compiled to WASM files in `target/wasm32-unknown-unknown/release/`
2. Copied to `app/static/nodes/` for serving
3. Registered in the browser via IndexedDB using the registry package

## Key Dependencies

**Frontend:**

- `@sveltejs/kit` - Application framework
- `@threlte/core` & `@threlte/extras` - Three.js Svelte integration
- `three` - 3D graphics library
- `tailwindcss` - CSS framework
- `comlink` - WebWorker RPC
- `idb` - IndexedDB wrapper
- `wabt` - WebAssembly binary toolkit

**Rust/WASM:**

- Language: Rust (compiled with plain cargo)
- Output: WebAssembly (wasm32-unknown-unknown target)
- Generic WASM wrapper for language-agnostic node development
- `glam` - Math library (Vec2, Vec3, Mat4, etc.)
- `nodarium_macros` - Custom procedural macros
- `nodarium_utils` - Shared node utilities

## Build Commands

From root directory:

```bash
# Install dependencies
pnpm i

# Build all WASM nodes (compiles Rust, copies to app/static)
pnpm build:nodes

# Build the app (builds UI library + SvelteKit app)
pnpm build:app

# Full build (nodes + app)
pnpm build

# Development
pnpm dev              # Run all dev commands in parallel
pnpm dev:nodes        # Watch nodes/, auto-rebuild on changes
pnpm dev:app_ui       # Watch app and UI package
pnpm dev_ui           # Watch UI package only
```

## Workspace Packages

The project uses pnpm workspaces with the following packages:

| Package            | Location           | Purpose                        |
| ------------------ | ------------------ | ------------------------------ |
| @nodarium/app      | app/               | Main SvelteKit application     |
| @nodarium/ui       | packages/ui/       | Reusable UI component library  |
| @nodarium/registry | packages/registry/ | Node registry with persistence |
| @nodarium/types    | packages/types/    | Shared TypeScript types        |
| @nodarium/utils    | packages/utils/    | Shared utilities               |
| nodarium macros    | packages/macros/   | Rust procedural macros         |

## Configuration Files

- `.dprint.jsonc` - Dprint formatter configuration
- `svelte.config.js` - SvelteKit configuration (app and ui)
- `vite.config.ts` - Vite bundler configuration
- `tsconfig.json` - TypeScript configuration (app and packages)
- `Cargo.toml` - Rust workspace with member packages
- `flake.nix` - Nix development environment

## Development Workflow

### Adding a New Node

1. Copy the `.template` directory in `nodes/max/plantarium/` to create a new node directory
2. Define node in `src/definition.json`
3. Implement logic in `src/lib.rs`
4. Build with `cargo build --release --target wasm32-unknown-unknown`
5. Test by dragging onto the node graph

### Modifying UI Components

1. Changes to `packages/ui/` automatically rebuild with watch mode
2. App imports from `@nodarium/ui`
3. Run `pnpm dev:app_ui` for hot reload

## Important Notes for AI Agents

1. **WASM Compilation**: Nodes require `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`)
2. **Cross-Compilation**: WASM build happens on host, not in containers/VMs
3. **Static Serving**: Compiled WASM files must exist in `app/static/nodes/` before dev server runs
4. **Workspace Dependencies**: Use `workspace:*` protocol for internal packages
5. **Threlte Version**: Uses Threlte 8.x, not 7.x (important for 3D component APIs)
6. **Svelte 5**: Project uses Svelte 5 with runes (`$state`, `$derived`, `$effect`)
7. **Tailwind 4**: Uses Tailwind CSS v4 with `@tailwindcss/vite` plugin
8. **IndexedDB**: Registry uses IDB for persistent node storage in browser
