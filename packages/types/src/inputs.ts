import { z } from 'zod';

/**
 * The unit an input is measured in, values are always stored in this unit: `m` for lengths, shown
 * in the unit of the project, and `rad` for angles, shown in degrees or radians as the user chooses
 */
export const InputUnitSchema = z.enum(['m', 'rad']);
export type InputUnit = z.infer<typeof InputUnitSchema>;

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
        z.literal('path'),
        z.literal('force'),
        z.literal('points'),
        z.literal('mesh'),
        z.literal('pattern')
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
  step: z.number().optional(),
  unit: InputUnitSchema.optional()
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

export const NodeInputCurveSchema = z.object({
  ...DefaultOptionsSchema.shape,
  type: z.literal('curve'),
  value: z.array(z.number()).optional()
});

/**
 * A text set on the node, like the rules of `lsystem`. The host sends it as one word per Unicode
 * code point, `[0, n + 1, ...code points, 1, 1]`. No node gives text, so it is never connected
 */
export const NodeInputTextSchema = z.object({
  ...DefaultOptionsSchema.shape,
  type: z.literal('text'),
  value: z.string().optional(),
  /** a field of several lines, Enter starts a new line */
  multiline: z.boolean().optional()
});

export const NodeInputBooleanSchema = z.object({
  ...DefaultOptionsSchema.shape,
  type: z.literal('boolean'),
  value: z.boolean().optional()
});

export const NodeInputSelectSchema = z.object({
  ...DefaultOptionsSchema.shape,
  type: z.literal('select'),
  options: z.array(
    z.union([z.string(), z.object({ value: z.number(), label: z.string() })])
  ).optional(),
  value: z.union([z.string(), z.number()]).optional()
});

export const NodeInputSeedSchema = z.object({
  ...DefaultOptionsSchema.shape,
  type: z.literal('seed'),
  value: z.number().optional()
});

// filled in by the host with the id of the graph node, it can't be wired or set
export const NodeInputCreatorSchema = z.object({
  ...DefaultOptionsSchema.shape,
  type: z.literal('creator'),
  value: z.number().optional()
});

export const NodeInputVec3Schema = z.object({
  ...DefaultOptionsSchema.shape,
  type: z.literal('vec3'),
  value: z.array(z.number()).optional(),
  unit: InputUnitSchema.optional()
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

export const NodeInputForceSchema = z.object({
  ...DefaultOptionsSchema.shape,
  type: z.literal('force'),
  value: z.array(z.number()).optional()
});

export const NodeInputPointsSchema = z.object({
  ...DefaultOptionsSchema.shape,
  type: z.literal('points'),
  value: z.array(z.number()).optional()
});

/** Only the group inputs of graphs before version 15, their migration makes them geometry */
export const NodeInputMeshSchema = z.object({
  ...DefaultOptionsSchema.shape,
  type: z.literal('mesh'),
  value: z.array(z.number()).optional()
});

/** A 2D pattern of values, like the stripes of `reaction_diffusion` */
export const NodeInputPatternSchema = z.object({
  ...DefaultOptionsSchema.shape,
  type: z.literal('pattern'),
  value: z.array(z.number()).optional()
});

export const NodeInputAnySchema = z.object({
  ...DefaultOptionsSchema.shape,
  type: z.literal('*'),
  value: z.any().optional()
});

export const NodeInputSchema = z.union([
  NodeInputSeedSchema,
  NodeInputCreatorSchema,
  NodeInputBooleanSchema,
  NodeInputFloatSchema,
  NodeInputColorSchema,
  NodeInputIntegerSchema,
  NodeInputShapeSchema,
  NodeInputCurveSchema,
  NodeInputTextSchema,
  NodeInputSelectSchema,
  NodeInputVec3Schema,
  NodeInputGeometrySchema,
  NodeInputPathSchema,
  NodeInputForceSchema,
  NodeInputPointsSchema,
  NodeInputMeshSchema,
  NodeInputPatternSchema,
  NodeInputAnySchema
]);

export type NodeInput = z.infer<typeof NodeInputSchema>;
