import { z } from 'zod';

const DefaultOptionsSchema = z.object({
  internal: z.boolean().optional(),
  external: z.boolean().optional(),
  setting: z.string().optional(),
  label: z.string().optional(),
  description: z.string().optional(),
  accepts: z
    .array(
      z.union([
        z.literal('float'),
        z.literal('integer'),
        z.literal('boolean'),
        z.literal('select'),
        z.literal('seed'),
        z.literal('vec3'),
        z.literal('geometry'),
        z.literal('path')
      ])
    )
    .optional(),
  hidden: z.boolean().optional()
});

export const NodeInputFloatSchema = z.object({
  ...DefaultOptionsSchema.shape,
  type: z.literal('float'),
  value: z.number().optional(),
  min: z.number().optional(),
  max: z.number().optional(),
  step: z.number().optional()
});

export const NodeInputColorSchema = z.object({
  ...DefaultOptionsSchema.shape,
  type: z.literal('color'),
  value: z.array(z.number()).optional()
});

export const NodeInputIntegerSchema = z.object({
  ...DefaultOptionsSchema.shape,
  type: z.literal('integer'),
  value: z.number().optional(),
  min: z.number().optional(),
  max: z.number().optional()
});

export const NodeInputShapeSchema = z.object({
  ...DefaultOptionsSchema.shape,
  type: z.literal('shape'),
  value: z.array(z.number()).optional()
});

export const NodeInputBooleanSchema = z.object({
  ...DefaultOptionsSchema.shape,
  type: z.literal('boolean'),
  value: z.boolean().optional()
});

export const NodeInputSelectSchema = z.object({
  ...DefaultOptionsSchema.shape,
  type: z.literal('select'),
  options: z.array(z.string()).optional(),
  value: z.string().optional()
});

export const NodeInputSeedSchema = z.object({
  ...DefaultOptionsSchema.shape,
  type: z.literal('seed'),
  value: z.number().optional()
});

export const NodeInputVec3Schema = z.object({
  ...DefaultOptionsSchema.shape,
  type: z.literal('vec3'),
  value: z.array(z.number()).optional()
});

export const NodeInputGeometrySchema = z.object({
  ...DefaultOptionsSchema.shape,
  type: z.literal('geometry'),
  value: z.array(z.number()).optional()
});

export const NodeInputPathSchema = z.object({
  ...DefaultOptionsSchema.shape,
  type: z.literal('path'),
  value: z.array(z.number()).optional()
});

export const NodeInputSchema = z.union([
  NodeInputSeedSchema,
  NodeInputBooleanSchema,
  NodeInputFloatSchema,
  NodeInputColorSchema,
  NodeInputIntegerSchema,
  NodeInputShapeSchema,
  NodeInputSelectSchema,
  NodeInputSeedSchema,
  NodeInputVec3Schema,
  NodeInputGeometrySchema,
  NodeInputPathSchema
]);

export type NodeInput = z.infer<typeof NodeInputSchema>;
