# Plantarium

Plantarium is the node system in `nodes/max/plantarium`. Its nodes generate plants. All of them use `nodarium_utils` (`packages/utils`), which implements the formats below, so node code rarely touches them directly.

## Numbers

A `float` or `integer` input is either a plain value or an **expression** produced by the `math` or `random` node:

```
[0, op, a, b]        math: a + b, a - b, a * b, a / b  (op 0-3)
[1, min, max, seed]  random value between min and max
```

`a`, `b`, `min` and `max` can be expressions themselves. Nodes read these inputs with a `Ctx`, created once per execution: `ctx.float` and `ctx.vec3`. Calling them again gives a new random value, which is how a single `random` node gives every stem a different length. A new `Ctx` starts the random values from the beginning, so a node gives the same result every time it runs. `integer`, `select`, `boolean` and `seed` inputs are never expressions, `int` reads them.

## Path

```
[0, stem_depth, ...points]    each point: x, y, z, thickness
```

## Geometry

```
[1, vertex_amount, face_amount, ...faces, ...positions, ...normals]
```

Faces are three vertex indices each, positions and normals three floats each.

## Instances

```
[2, vertex_amount, face_amount, instance_amount, stem_depth, ...faces, ...positions, ...normals, ...matrices]
```

One 4x4 matrix per instance.

## Lists

Nodes can return several of these at once. They are nested with the bracket encoding from `packages/utils/src/flatTree.ts`. The `output` node's result is what the app renders.
