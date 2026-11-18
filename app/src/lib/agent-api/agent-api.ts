import type { GraphManager } from '#lib/graph-interface/index.ts';
import type { RemoteNodeRegistry } from '#lib/node-registry/index.ts';
import { type GraphIssue, type RuntimeErrorInfo, validateGraph } from '@nodarium/runtime';
import { templates } from '@nodarium/runtime/templates';
import { type Graph, inputAccepts, type NodeInstance } from '@nodarium/types';
import type { BenchmarkResult, BuildResult, NodariumApi, ScreenshotView, Vec3 } from './types';
import { when } from './when.svelte';

/** One build of the page, as `update()` of the page saw it */
export type BuildRecord = {
  graph: Graph;
  result: Int32Array;
  error?: RuntimeErrorInfo;
  vertices: number;
  faces: number;
  bounds?: { min: Vec3; max: Vec3 };
  ms: number;
  nodeTimings: Record<number, number>;
};

/** What the page gives the API, every method is something the page already does */
export type AgentApiHost = {
  app: { version: string; sha: string };
  /** the manager of the open project, a new project gets a new one */
  manager(): GraphManager | undefined;
  /** reactive: no build running or waiting, no project being opened */
  isIdle(): boolean;
  lastBuild(): BuildRecord | undefined;
  /** builds a graph and shows it, without changing the open one */
  build(graph: Graph): Promise<void>;
  importGraph(
    input: unknown,
    options: { title?: string }
  ): Promise<{ id: number; issues: GraphIssue[] }>;
  createProject(graph: Graph, title: string): number;
  registry: RemoteNodeRegistry;
  /** the point of the graph in the middle of the editor */
  viewCenter(): [number, number];
  renderViews(result: Int32Array, size: number, views: ScreenshotView[]): Promise<Blob | undefined>;
};

const VIEWS: ScreenshotView[] = ['default', 'front', 'side', 'top'];

function label(node: { id: number; type: string }) {
  return `${node.type.split('/').pop()}#${node.id}`;
}

function median(values: number[]) {
  const sorted = [...values].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
}

function stdev(values: number[]) {
  const mean = values.reduce((a, b) => a + b, 0) / values.length;
  return Math.sqrt(values.reduce((sum, v) => sum + (v - mean) ** 2, 0) / values.length);
}

function plain<T>(value: T): T {
  return JSON.parse(JSON.stringify(value));
}

/** The definitions of every collection of every user, from their listings, no wasm */
async function loadAllDefinitions(registry: RemoteNodeRegistry) {
  const collections = await registry.listCollections();
  await Promise.all(collections.map((id) => registry.loadCollection(id)));
}

export function createAgentApi(host: AgentApiHost): NodariumApi {
  let ready: Promise<void> | undefined;

  const idle = () => when(host.isIdle);

  function manager() {
    const m = host.manager();
    if (!m) throw new Error('No graph is open yet, await nodarium.ready first');
    return m;
  }

  function nodeOf(m: GraphManager, id: number) {
    const node = m.getNode(id);
    if (!node) throw new Error(`The open graph has no node ${id}`);
    return node;
  }

  function checkInputs(type: string, inputs: Record<string, unknown>, keys: string[]) {
    const unknown = keys.filter((key) => !(key in inputs));
    if (unknown.length) {
      throw new Error(
        `${type} has no input ${unknown.map((k) => `"${k}"`).join(', ')}, it has ${
          Object.keys(inputs).join(', ') || 'none'
        }`
      );
    }
  }

  function definitionMap() {
    return new Map(host.registry.getAllNodes().map((d) => [d.id as string, d]));
  }

  function resultOf(build: BuildRecord, issues: GraphIssue[]): BuildResult {
    return {
      ok: !build.error && !issues.some((issue) => issue.severity === 'error'),
      issues,
      ...(build.error && { error: build.error }),
      vertices: build.vertices,
      faces: build.faces,
      ...(build.bounds && { bounds: build.bounds }),
      ms: { total: build.ms, nodes: { ...build.nodeTimings } }
    };
  }

  function lastResult() {
    const build = host.lastBuild();
    return build && resultOf(build, validateGraph(build.graph, definitionMap()));
  }

  function builtResult() {
    const result = lastResult();
    if (!result) throw new Error('Nothing was built yet');
    return result;
  }

  function setSettings(m: GraphManager, settings: Record<string, unknown>) {
    const unknown = Object.keys(settings).filter((key) => !(key in m.settingTypes));
    if (unknown.length) {
      throw new Error(
        `The graph has no setting ${unknown.map((k) => `"${k}"`).join(', ')}, it has ${
          Object.keys(m.settingTypes).join(', ')
        }`
      );
    }
    const changed = Object.entries(settings).filter(([key, value]) =>
      JSON.stringify(m.settings[key]) !== JSON.stringify(value)
    );
    if (!changed.length) return;
    for (const [key, value] of changed) m.settings[key] = value;
    m.commit();
  }

  /** Waits until project `id` is open and built, a failed load gives a result without a build */
  async function opened(id: number, issues?: GraphIssue[]): Promise<BuildResult> {
    await when(() =>
      host.isIdle() && (host.manager()?.id === id || host.manager()?.status === 'error')
    );
    if (host.manager()?.status === 'error') {
      return {
        ok: false,
        issues: issues ?? [],
        error: { message: 'The graph could not be opened, the toast of the page says why' },
        vertices: 0,
        faces: 0,
        ms: { total: 0, nodes: {} }
      };
    }
    const result = builtResult();
    if (!issues) return result;
    return { ...result, issues, ok: !result.error && !issues.some((i) => i.severity === 'error') };
  }

  const api: NodariumApi = {
    version: 1,
    app: host.app,

    get ready() {
      ready ??= idle().then(() => loadAllDefinitions(host.registry));
      return ready;
    },
    idle,

    graph: {
      get: () => manager().serialize(),
      async load(graph, options = {}) {
        const { id, issues } = await host.importGraph(graph, options);
        return opened(id, issues);
      },
      async loadTemplate(id) {
        const template = templates.find((t) => t.id === id);
        if (!template) {
          throw new Error(
            `There is no template "${id}", there are ${templates.map((t) => t.id).join(', ')}`
          );
        }
        return opened(host.createProject(await template.load(), template.name));
      },
      addNode(type, props = {}) {
        const m = manager();
        const definition = m.registry.getNode(type);
        if (!definition) throw new Error(`There is no node type "${type}"`);
        checkInputs(type, definition.inputs ?? {}, Object.keys(props));
        const node = m.createNode({
          type: type as NodeInstance['type'],
          position: host.viewCenter(),
          props: props as NodeInstance['props']
        });
        if (!node) throw new Error(`The node "${type}" could not be added`);
        return node.id;
      },
      removeNode(id) {
        const m = manager();
        m.removeNode(nodeOf(m, id));
      },
      connect(from, output, to, input) {
        const m = manager();
        const source = nodeOf(m, from);
        const target = nodeOf(m, to);
        const outputs = m.getNodeSockets(source).outputs;
        if (!Number.isInteger(output) || output < 0 || output >= outputs.length) {
          throw new Error(`${label(source)} has ${outputs.length} outputs, there is no ${output}`);
        }
        const inputs = m.getNodeSockets(target).inputs;
        checkInputs(target.type, inputs, [input]);
        if (inputs[input].internal) {
          throw new Error(`"${input}" of ${label(target)} can't be connected`);
        }
        if (!inputAccepts(inputs[input], outputs[output])) {
          throw new Error(
            `"${input}" of ${label(target)} takes ${inputs[input].type}, output ${output} of ${
              label(source)
            } is ${outputs[output]}`
          );
        }
        if (source === target || m.getChildren(target).includes(source)) {
          throw new Error(`${label(source)} → ${label(target)} would make a cycle`);
        }
        m.createEdge(source, output, target, input);
      },
      disconnect(to, input) {
        const m = manager();
        nodeOf(m, to);
        const edge = m.edges.find((e) => e[2].id === to && e[3] === input);
        if (edge) m.removeEdge(edge);
      },
      setProp(id, key, value) {
        const m = manager();
        const node = nodeOf(m, id);
        checkInputs(node.type, m.getNodeSockets(node).inputs, [key]);
        node.props = { ...node.props, [key]: value } as NodeInstance['props'];
        m.commit();
      },
      setSettings: (settings) => setSettings(manager(), settings),
      format: () => manager().formatNodes(),
      undo: () => manager().undo(),
      redo: () => manager().redo()
    },

    nodes: {
      list: () => plain(host.registry.getAllNodes()),
      get(type) {
        const definition = host.registry.getAllNodes().find((d) => d.id === type);
        return definition && plain(definition);
      }
    },

    async run({ seed } = {}) {
      const m = manager();
      if (seed !== undefined) setSettings(m, { seed });
      m.execute();
      await idle();
      return builtResult();
    },

    lastResult,

    async benchmark({ runs = 20, warmups = 3 } = {}) {
      await idle();
      const graph = manager().serialize();
      const samples: number[] = [];
      try {
        for (let i = 0; i < warmups + runs; i++) {
          const seed = Math.floor(Math.random() * 100000000);
          const start = performance.now();
          await host.build({ ...graph, settings: { ...graph.settings, seed } });
          const ms = performance.now() - start;
          const error = host.lastBuild()?.error;
          if (error) throw new Error(error.message);
          if (i >= warmups) samples.push(ms);
        }
      } finally {
        await host.build(manager().serialize());
      }
      return { samples, median: median(samples), stdev: stdev(samples) } satisfies BenchmarkResult;
    },

    viewer: {
      async screenshot(
        { size = 1024, views = ['default'] }: { size?: number; views?: ScreenshotView[] } = {}
      ) {
        const unknown = views.filter((view) => !VIEWS.includes(view));
        if (unknown.length || views.length === 0) {
          throw new Error(`views takes some of ${VIEWS.join(', ')}`);
        }
        if (!Number.isInteger(size) || size < 16 || size > 4096) {
          throw new Error('size is a whole number of pixels from 16 to 4096');
        }
        await idle();
        const build = host.lastBuild();
        const blob = build && !build.error
          ? await host.renderViews(build.result, size, views)
          : undefined;
        if (!blob) throw new Error('The last build shows nothing to take a picture of');
        return blob;
      }
    }
  };
  return api;
}
