import { z } from 'zod';
import { NodeInputSchema } from './inputs';

export type Box = {
  minX: number;
  maxX: number;
  minY: number;
  maxY: number;
};

export const NodeIdSchema = z
  .string()
  .regex(/^[^/]+\/[^/]+\/[^/]+$/, 'Invalid NodeId format')
  .transform((value) => value as `${string}/${string}/${string}`);

export type NodeId = z.infer<typeof NodeIdSchema>;

export type NodeRuntimeState = {
  mesh?: any;
  // The registry definition. The sockets of group nodes depend on their group,
  // read them through GraphManager.getNodeType()
  type?: NodeDefinition;
  downX?: number;
  downY?: number;
  x?: number;
  y?: number;
  ref?: HTMLElement;
  isMoving?: boolean;
};

/** A node of the editor, which places every node before it shows it */
export type NodeInstance = {
  state: NodeRuntimeState;
  position: [number, number];
} & SerializedNode;

/**
 * An output of a node, either only its type or its type and a label.
 * The label tells outputs of the same type apart on the node.
 */
export const NodeOutputSchema = z.union([
  z.string(),
  z.object({
    type: z.string(),
    label: z.string().optional()
  })
]);

export type NodeOutput = z.infer<typeof NodeOutputSchema>;

export function getOutputType(output: NodeOutput | undefined): string | undefined {
  return typeof output === 'string' ? output : output?.type;
}

export function getOutputLabel(output: NodeOutput | undefined): string | undefined {
  if (output === undefined) return undefined;
  return typeof output === 'string' ? output : output.label ?? output.type;
}

export const NodeDefinitionSchema = z.object({
  id: NodeIdSchema,
  inputs: z.record(z.string(), NodeInputSchema).optional(),
  outputs: z.array(NodeOutputSchema).optional(),
  meta: z
    .object({
      description: z.string().optional(),
      title: z.string().optional()
    })
    .optional()
});

export const NodeSchema = z.object({
  id: z.number(),
  type: NodeIdSchema,
  props: z
    .record(z.string(), z.union([z.number(), z.array(z.number())]))
    .optional(),
  // hidden inputs that are shown on this node
  exposed: z.array(z.string()).optional(),
  muted: z.boolean().optional(),
  meta: z
    .object({
      title: z.string().optional(),
      lastModified: z.string().optional()
    })
    .optional(),
  // where the node sits in the editor, a node without one is placed when the graph is loaded
  position: z.tuple([z.number(), z.number()]).optional()
});

export type SerializedNode = z.infer<typeof NodeSchema>;

/**
 * `len` i32 words at byte offset `ptr` inside a node module's memory.
 * Only valid until that module is reset.
 */
export type WasmSlice = {
  memory: WebAssembly.Memory;
  ptr: number;
  len: number;
};

export type NodeValue = Int32Array | WasmSlice;

export type NodeDefinition = z.infer<typeof NodeDefinitionSchema> & {
  /**
   * Receives one value per input, in definition order. Returns one value per
   * output, a node with a single output may return that value directly.
   */
  execute(inputs: NodeValue[]): NodeValue | NodeValue[];
  /** Frees all results of previous executions, invalidating their WasmSlices */
  reset?(): void;
  /** Frees the memory of the node, the next execute starts with a fresh one */
  release?(): void;
  /** Bytes of memory the node holds right now */
  memoryBytes?(): number;
};

export type Socket = {
  node: NodeInstance;
  index: number | string;
  position: [number, number];
};

export type Edge = [NodeInstance, number, NodeInstance, string];

const SerializedEdgeSchema = z.tuple([z.number(), z.number(), z.number(), z.string()]);

export type SerializedEdge = z.infer<typeof SerializedEdgeSchema>;

/** The colours a frame can have, every theme defines a `--color-frame-<name>` for each */
export const FRAME_COLORS = ['gray', 'green', 'brown', 'blue', 'red', 'violet'] as const;
export type FrameColor = (typeof FRAME_COLORS)[number];

/**
 * A labelled rectangle behind some nodes of one level. It computes nothing, its rectangle is the
 * box around its nodes.
 */
export const FrameSchema = z.object({
  /** unique among the frames of its level */
  id: z.number(),
  label: z.string(),
  // a colour this version doesn't know is read as gray, not as a broken file
  color: z.enum(FRAME_COLORS).optional().catch(undefined),
  /** the ids of nodes of the same level, a node is in at most one frame */
  nodes: z.array(z.number())
});

export type Frame = z.infer<typeof FrameSchema>;

export const GroupSchema = z.object({
  id: z.number(),
  name: z.string().optional(),
  nodes: z.array(NodeSchema),
  edges: z.array(z.tuple([z.number(), z.number(), z.number(), z.string()])),
  inputs: z.record(z.string(), NodeInputSchema).optional(),
  outputs: z.array(z.object({
    type: z.string(),
    label: z.string().optional()
  })).optional(),
  frames: z.array(FrameSchema).optional()
});

export type GroupDefinition = z.infer<typeof GroupSchema>;

export const ProjectUnitSchema = z.enum(['mm', 'cm', 'm']);
export type ProjectUnit = z.infer<typeof ProjectUnitSchema>;

export const GraphSchema = z.object({
  id: z.number(),
  version: z.number().int().nonnegative().optional(),
  meta: z
    .object({
      title: z.string().optional(),
      lastModified: z.string().optional(),
      // the unit lengths are displayed in, values are always stored in meters
      unit: ProjectUnitSchema.optional()
    })
    .optional(),
  settings: z.record(z.string(), z.any()).optional(),
  nodes: z.array(NodeSchema),
  edges: z.array(SerializedEdgeSchema),
  groups: z.array(GroupSchema),
  frames: z.array(FrameSchema).optional()
});

export type Graph = z.infer<typeof GraphSchema>;

export const ProjectFileSchema = z.object({
  format: z.literal('nodarium-project'),
  version: z.literal(1),
  graph: GraphSchema,
  preview: z.string().startsWith('data:image/').optional()
});

export type ProjectFile = z.infer<typeof ProjectFileSchema>;
