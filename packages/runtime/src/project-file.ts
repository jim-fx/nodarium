import { GraphSchema, type ProjectFile, ProjectFileSchema } from '@nodarium/types';
import { z } from 'zod';
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

// the id is the one of a project in the app, a graph without groups has none
const GraphFileSchema = GraphSchema.extend({
  id: GraphSchema.shape.id.optional(),
  groups: GraphSchema.shape.groups.optional()
}).meta({
  id: 'Graph',
  title: 'Nodarium graph',
  description:
    'A graph of nodes: every node has an id, a type like max/plantarium/stem and the values of its inputs in props. An edge is [from node, output index, to node, input name].'
});

const ProjectFileInputSchema = ProjectFileSchema.extend({ graph: GraphFileSchema }).meta({
  id: 'ProjectFile',
  title: 'Nodarium project file',
  description: 'A project as File → Export .json of the app writes it: the graph and a preview.'
});

/**
 * The JSON Schema of what `parseProjectFile` reads: a project file or a bare graph. It says what
 * the file looks like, `validateGraph` whether its nodes and edges make sense.
 */
export function graphFileJsonSchema() {
  const schema = z.toJSONSchema(z.union([ProjectFileInputSchema, GraphFileSchema]), {
    io: 'input',
    override: ({ jsonSchema }) => {
      // zod's name for startsWith is no format of JSON Schema, the pattern says the same
      if (jsonSchema.format === 'starts_with') delete jsonSchema.format;
    }
  });
  return {
    ...schema,
    title: 'Nodarium graph file',
    description: 'A project file or a bare graph, as File → Import .json of the app reads it.'
  };
}
