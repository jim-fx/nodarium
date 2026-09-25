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
  depth?: number;
  mesh?: any;
  parents?: NodeInstance[];
  children?: NodeInstance[];
  inputNodes?: Record<string, NodeInstance>;
  type?: NodeDefinition;
  downX?: number;
  downY?: number;
  x?: number;
  y?: number;
  ref?: HTMLElement;
  visible?: boolean;
  isMoving?: boolean;
};

export type NodeInstance = {
  state: NodeRuntimeState;
} & SerializedNode;

export const NodeDefinitionSchema = z.object({
  id: NodeIdSchema,
  inputs: z.record(z.string(), NodeInputSchema).optional(),
  outputs: z.array(z.string()).optional(),
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
  meta: z
    .object({
      title: z.string().optional(),
      lastModified: z.string().optional()
    })
    .optional(),
  position: z.tuple([z.number(), z.number()])
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
  /** Receives one value per input, in definition order */
  execute(inputs: NodeValue[]): NodeValue;
  /** Frees all results of previous executions, invalidating their WasmSlices */
  reset?(): void;
};

export type Socket = {
  node: NodeInstance;
  index: number | string;
  position: [number, number];
};

export type Edge = [NodeInstance, number, NodeInstance, string];

const SerializedEdgeSchema = z.tuple([z.number(), z.number(), z.number(), z.string()]);

export type SerializedEdge = z.infer<typeof SerializedEdgeSchema>;

export const GroupSchema = z.object({
  id: z.number(),
  name: z.string().optional(),
  nodes: z.array(NodeSchema),
  edges: z.array(z.tuple([z.number(), z.number(), z.number(), z.string()])),
  inputs: z.record(z.string(), NodeInputSchema).optional(),
  outputs: z.array(z.object({
    type: z.string(),
    label: z.string().optional()
  })).optional()
});

export type GroupDefinition = z.infer<typeof GroupSchema>;

export const GraphSchema = z.object({
  id: z.number(),
  meta: z
    .object({
      title: z.string().optional(),
      lastModified: z.string().optional()
    })
    .optional(),
  settings: z.record(z.string(), z.any()).optional(),
  nodes: z.array(NodeSchema),
  edges: z.array(SerializedEdgeSchema),
  groups: z.array(GroupSchema)
});

export type Graph = z.infer<typeof GraphSchema>;
