import { GraphSchema, type ProjectFile, ProjectFileSchema } from '@nodarium/types';
import { migrateGraph } from './graph-migrations';

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

// The id is reassigned on import and older graphs were saved without groups
function withGraphDefaults(graph: unknown) {
  return isObject(graph) ? { id: 0, groups: [], ...graph } : graph;
}

function parseOrThrow<T>(
  result: { success: true; data: T } | {
    success: false;
    error: { issues: { path: PropertyKey[]; message: string }[] };
  }
): T {
  if (result.success) return result.data;
  const issues = result.error.issues.map((issue) =>
    `${issue.path.map(String).join('.') || 'file'}: ${issue.message}`
  );
  throw new Error(`Invalid project file (${issues.join(', ')})`);
}

/** Accepts a project file or a bare graph (the format before project files existed) */
export function parseProjectFile(input: unknown): ProjectFile {
  if (isObject(input) && 'format' in input) {
    const file = parseOrThrow(
      ProjectFileSchema.safeParse({ ...input, graph: withGraphDefaults(input.graph) })
    );
    return { ...file, graph: migrateGraph(file.graph) };
  }
  const graph = parseOrThrow(GraphSchema.safeParse(withGraphDefaults(input)));
  return { format: 'nodarium-project', version: 1, graph: migrateGraph(graph) };
}
