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

An `integer` input that isn't set by a setting is a plain value or the expression of the `random_int` node:

```
[4, min, max, seed]  random_int: a whole number from min to max, both included, every one equally likely
```

`a`, `b`, `min`, `max` and `value` can be expressions themselves. The points of a curve are plain values, sorted by x from 0 to 1. Between them the curve is a monotone cubic, so it never overshoots a point. Nodes read these inputs with a `Ctx`, created once per execution: `ctx.float`, `ctx.vec3` and `ctx.int`. Calling them again gives a new random value, which is how a single `random` node gives every stem a different length, and a `random_int` node every branch a different number of leaves. A new `Ctx` starts the random values from the beginning, so a node gives the same result every time it runs. `select`, `boolean` and `seed` inputs and `integer` inputs set by a setting (the resolutions) are never expressions, `int` reads them.

`branch` and `instance` set `ctx.alpha` before they evaluate the inputs of each branch or instance, for example its `length` or `size`: alpha is where it sits on its parent path. `stem` and `branch` read `thickness` with `ctx.float_along` at every point of the path they create, there alpha goes along that path. `float_along` draws random values once, so a `random` thickness stays the same along the path. The path is then multiplied by `1 - alpha * taper`: `taper` 1 (the default) makes a cone, 0 leaves the profile to the thickness input. Everywhere else alpha is 0. The second word of the alpha expression keeps it from being read as a plain value.

## Path

```
[0, stem_depth, id, parent_id, parent_alpha, ...points]    each point: x, y, z, thickness
```

`id` is unique among the paths of a list. `parent_id` is the `id` of the path this one grows on, `-1` for a path that has no parent. `parent_alpha` is an `f32`, where along the length of the parent (0 to 1) the path is attached. `stem` gives its paths the ids 0, 1, 2 and so on, `branch` gives every new branch the next free id and links it to the path it grows on. `join` shifts the ids of each list it joins, so they stay unique. A node that removes a path leaves the links of its children pointing at nothing, nodes that follow a parent must ignore a `parent_id` they can't find. A node that shortens a path changes what `parent_alpha` of its children means.

## Force

```
[3, kind, x, y, z, strength, radius, falloff]    all but the first two words are f32
```

A `force` output is a list of these, so a later node can combine forces. `kind` 0 is a point force: it pulls towards `x, y, z`, a negative `strength` pushes away. It reaches `radius` far, `falloff` 0 keeps the full strength up to the radius and 1 fades it smoothly to nothing at the radius. `Force::sample` in `nodarium_utils` gives the push at a position, `decode_forces` and `sample_forces` read a list of them. `apply_force` bends paths with them: the base of a path stays, the segments keep their length. Paths follow the spot on their parent they grow on, so a bent stem carries its branches. `PathStructure` and `follow_parents` in `nodarium_utils::geometry` do that for any node that moves paths.

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
