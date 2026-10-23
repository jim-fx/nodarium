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
[5, field]           path_info: a value of the path the node works on, field is the option of the node:
                     0 position, 1 index, 2 count, 3 depth, 4 parent position, 5 length
```

An `integer` input that isn't set by a setting is a plain value or the expression of the `random_int` node:

```
[4, min, max, seed]  random_int: a whole number from min to max, both included, every one equally likely
```

`a`, `b`, `min`, `max` and `value` can be expressions themselves. The points of a curve are plain values, sorted by x from 0 to 1. Between them the curve is a monotone cubic, so it never overshoots a point. Nodes read these inputs with a `Ctx`, created once per execution: `ctx.float`, `ctx.vec3` and `ctx.int`. Calling them again gives a new random value, which is how a single `random` node gives every stem a different length, and a `random_int` node every branch a different number of leaves. A new `Ctx` starts the random values from the beginning, so a node gives the same result every time it runs. `set_path` (and `seed_path`, which `stem` calls for the stems it is about to make) restarts the random values from the `id` of the path, so a path draws the same values whether or not other paths come before it in the list. `select`, `boolean` and `seed` inputs and `integer` inputs set by a setting (the resolutions) are never expressions, `int` reads them.

`branch` and `instance` set `ctx.alpha` before they evaluate the inputs of each branch or instance, for example its `length` or `size`: alpha is where it sits on its parent path. `stem` and `branch` read `thickness` with `ctx.float_along` at every point of the path they create, there alpha goes along that path. `float_along` draws random values once, so a `random` thickness stays the same along the path. The path is then multiplied by `1 - alpha * taper`: `taper` 1 (the default) makes a cone, 0 leaves the profile to the thickness input. Everywhere else alpha is 0. The second word of the alpha expression keeps it from being read as a plain value.

Alpha is only one number that changes from place to place, so it can't tell a node about the path itself. `stem`, `branch`, `instance`, `gravity`, `rotate`, `noise` and `apply_force` also call `ctx.set_path` (or `ctx.set_path_info` for a stem that doesn't exist yet) before they read the inputs of each path. It sets `ctx.path`, what the `path_info` node reads: the `index` in the header of the path (the number among the paths its creator made on the same parent, it stays when other paths come or go), the place of the path among the paths the node works on (the `position`) and how many there are, its depth, where on its parent it grows (the `parent_alpha` of its link, 0 for a stem) and its length. `set_path` also sets alpha to the position of the path, from 0 at the first to 1 at the last, so `alpha` reads the same as `path_info` with position until a node sets it to something else. A `branch` or `instance` works on the path it places its branches or instances on, so a `branch` on the points of a whorl reads where the whorl sits on its stem. A `path_info` value doesn't change along a path or from one branch to the next, only from path to path.

## Path

```
[0, stem_depth, id, parent_id, parent_alpha, creator, index, ...points]    each point: x, y, z, thickness
```

The header says where the path sits in the plant, so a node, the viewer or a material can pick any part of it:

- `creator` is the id of the graph node that made the path. The host fills it into the hidden `creator` input of `stem` and `branch` (type `creator`, like `seed` it can't be wired or set), so every path of a node has the same one, `creator == X` selects everything one node made.
- `parent_id` is the `id` of the path this one grows on, `-1` for a path that has no parent. `parent_id == X` selects the children of a path, walking up the parents (`pathLineage` in `@nodarium/utils`) the descendants.
- `index` is the number of the path among the paths its creator made on the same parent, the first is 0.
- `id` is a hash of `creator`, `parent_id` and `index` (`path_id` in `nodarium_utils::geometry`, never negative). It only depends on these three, so it is the same in every run and doesn't change when other paths come or go: more stems, another amount of branches on a different path, a node somewhere else in the graph. Changing the amount of a node only adds or removes the paths with the highest indexes, a branch that is skipped leaves a gap in the indexes instead of renumbering the rest. Two paths of a list never share an id (unless the same list is joined twice, see `join`).
- `parent_alpha` is an `f32`, where along the length of the parent (0 to 1) the path is attached.

`stem` makes paths without a parent, `branch` makes every new branch with the path it grows on as parent. `join` keeps the ids as they are, only a list that clashes with one joined before it (the same list joined twice) gets new ids, parents inside it follow. A node that removes a path leaves the links of its children pointing at nothing, nodes that follow a parent must ignore a `parent_id` they can't find. A node that shortens a path changes what `parent_alpha` of its children means. A node that copies a path keeps its header, so the ids survive `gravity`, `rotate`, `noise`, `snap`, `avoid` and `apply_force`.

## Force

```
[3, kind, x, y, z, strength, radius, falloff]    all but the first two words are f32
```

A `force` output is a list of these, so a later node can combine forces. `kind` 0 is a point force: it pulls towards `x, y, z`, a negative `strength` pushes away. It reaches `radius` far, `falloff` 0 keeps the full strength up to the radius and 1 fades it smoothly to nothing at the radius. `Force::sample` in `nodarium_utils` gives the push at a position, `decode_forces` and `sample_forces` read a list of them. `apply_force` bends paths with them: the base of a path stays, the segments keep their length. Paths follow the spot on their parent they grow on, so a bent stem carries its branches. `PathStructure` and `follow_parents` in `nodarium_utils::geometry` do that for any node that moves paths.

## Geometry

```
[1, vertex_amount, face_amount, path_id, ...faces, ...positions, ...normals]
```

Faces are three vertex indices each, positions and normals three floats each. `path_id` is the `id` of the path an extruded geometry was made from (`output` extrudes every path), `-1` (`NO_PATH`) for geometry that doesn't come from a path, like a leaf or a box. The viewer finds the header of the path (creator, parent) with this id.

## Instances

```
[2, vertex_amount, face_amount, instance_amount, stem_depth, ...faces, ...positions, ...normals, ...matrices, ...path_ids]
```

One 4x4 matrix and one `path_id` per instance, the `id` of the path the instance sits on (`-1` if none). `instance` writes the path each leaf is placed on, so leaves can be picked by the part of the plant they sit on like the paths themselves. `readPathLink`, `readGeometryPathId` and `readInstancePathIds` in `@nodarium/utils` read these.

## Lists

Nodes can return several of these at once. They are nested with the bracket encoding from `packages/utils/src/flatTree.ts`. The `output` node's result is what the app renders.
