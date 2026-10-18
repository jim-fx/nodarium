# Plantarium

Plantarium is the node system in `nodes/max/plantarium`. Its nodes generate plants. All of them use `nodarium_utils` (`packages/utils`), which implements the formats below, so node code rarely touches them directly.

## Numbers

A `float` input is either a plain value or an **expression** produced by the `math`, `random`, `alpha` or `curve` node:

```
[0, op, a, b]        math: a + b, a - b, a * b, a / b  (op 0-3)
[1, min, max, seed]  random value between min and max
[2, 0]               alpha: 0 at the base to 1 at the tip, see below
[3, value, min, max, x0, y0, x1, y1, ...]
                     curve: min + curve(value) * (max - min), value clamped to 0-1
```

`a`, `b`, `min`, `max` and `value` can be expressions themselves. The points of a curve are plain values, sorted by x from 0 to 1. Between them the curve is a monotone cubic, so it never overshoots a point. Nodes read these inputs with a `Ctx`, created once per execution: `ctx.float` and `ctx.vec3`. Calling them again gives a new random value, which is how a single `random` node gives every stem a different length. A new `Ctx` starts the random values from the beginning, so a node gives the same result every time it runs. `integer`, `select`, `boolean` and `seed` inputs are never expressions, `int` reads them.

`branch` and `instance` set `ctx.alpha` before they evaluate the inputs of each branch or instance, for example its `length` or `size`: alpha is where it sits on its parent path. `stem` and `branch` read `thickness` with `ctx.float_along` at every point of the path they create, there alpha goes along that path. `float_along` draws random values once, so a `random` thickness stays the same along the path. The path is then multiplied by `1 - alpha * taper`: `taper` 1 (the default) makes a cone, 0 leaves the profile to the thickness input. Everywhere else alpha is 0. The second word of the alpha expression keeps it from being read as a plain value.

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
