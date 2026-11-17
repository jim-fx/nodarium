import type { GraphIssue, RuntimeErrorInfo } from '@nodarium/runtime';
import type { Graph, NodeDefinition } from '@nodarium/types';

export type { GraphIssue, RuntimeErrorInfo } from '@nodarium/runtime';
export type { Graph, NodeDefinition } from '@nodarium/types';

export type Vec3 = [number, number, number];

/** What a build of the open graph gave */
export type BuildResult = {
  /** the build didn't fail and the graph has no issue of the severity `error` */
  ok: boolean;
  /** what `validateGraph()` finds in the graph that was built */
  issues: GraphIssue[];
  /** the node that failed, as the error log has it */
  error?: RuntimeErrorInfo;
  vertices: number;
  faces: number;
  /** of everything the viewer shows, in meters, missing when it shows nothing */
  bounds?: { min: Vec3; max: Vec3 };
  /** the time of the build, and of every node */
  ms: { total: number; nodes: Record<number, number> };
};

export type BenchmarkResult = { samples: number[]; median: number; stdev: number };

/** `window.nodarium`, see docs/AGENT_API.md */
export interface NodariumApi {
  version: 1;
  app: { version: string; sha: string };

  /** resolves once the app has opened its project, built it and knows all its nodes */
  readonly ready: Promise<void>;
  /** resolves once no build is running or waiting and no project is being opened */
  idle(): Promise<void>;

  graph: {
    /** the open graph as it is saved */
    get(): Graph;
    /** opens the graph or project file as a new project, its issues are those of an import */
    load(graph: unknown, options?: { title?: string }): Promise<BuildResult>;
    /** opens a template as a new project, as the project browser does */
    loadTemplate(id: string): Promise<BuildResult>;
    /** adds a node in the middle of the editor and returns its id */
    addNode(type: string, props?: Record<string, unknown>): number;
    removeNode(id: number): void;
    /** connects output `output` of node `from` to the input `input` of node `to` */
    connect(from: number, output: number, to: number, input: string): void;
    disconnect(to: number, input: string): void;
    setProp(id: number, key: string, value: unknown): void;
    setSettings(settings: Record<string, unknown>): void;
    /** lays out the nodes in columns, as "Format nodes" of the editor does */
    format(): void;
    undo(): void;
    redo(): void;
  };

  nodes: { list(): NodeDefinition[]; get(type: string): NodeDefinition | undefined };

  /** builds the open graph, with `seed` as its seed, and waits for it */
  run(options?: { seed?: number }): Promise<BuildResult>;
  lastResult(): BuildResult | undefined;
  /** builds the open graph `runs` times with random seeds, the graph itself is not changed */
  benchmark(options?: { runs?: number; warmups?: number }): Promise<BenchmarkResult>;
}

declare global {
  interface Window {
    nodarium?: NodariumApi;
  }
}
