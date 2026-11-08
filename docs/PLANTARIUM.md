# Plantarium

Plantarium is the node system in `nodes/max/plantarium`. Its nodes generate plants. All of them use `nodarium_utils` (`packages/utils`), which implements the formats below, so node code rarely touches them directly.

## Numbers

A `float` input is either a plain value or an **expression** produced by the `math`, `random`, `alpha` or `curve` node:

```
[0, op, a, b]        math: a + b, a - b, a * b, a / b  (op 0-3)
[1, min, max, seed, uniform, per_path, salt]
                     random value between min and max, the last three are optional
[2, 0]               alpha: 0 at the base to 1 at the tip, see below
[3, value, min, max, x0, y0, x1, y1, ...]
                     curve: min + curve(value) * (max - min), value clamped to 0-1
[5, field]           path_info: a value of the path the node works on, field is the option of the node:
                     0 position, 1 index, 2 count, 3 depth, 4 parent position, 5 length
```

An `integer` input that isn't set by a setting is a plain value or the expression of the `random_int` or `round` node:

```
[4, min, max, seed]  random_int: a whole number from min to max, both included, every one equally likely
[6, value]           round: a float expression rounded to the nearest whole number
```

`a`, `b`, `min`, `max` and `value` can be expressions themselves. The points of a curve are plain values, sorted by x from 0 to 1. Between them the curve is a monotone cubic, so it never overshoots a point. Nodes read these inputs with a `Ctx`, created once per execution: `ctx.float`, `ctx.vec3` and `ctx.int`. Calling them again gives a new random value, which is how a single `random` node gives every stem a different length, and a `random_int` node every branch a different number of leaves. A new `Ctx` starts the random values from the beginning, so a node gives the same result every time it runs. `set_path` (and `seed_path`, which `stem` calls for the stems it is about to make) restarts the random values from the `id` of the path, so a path draws the same values whether or not other paths come before it in the list. A `random` with `per_path` ("once per path") is the exception: its value only depends on the seed, the id of the path that was seeded last and the node that made it (`salt`, its creator id), and it draws nothing, so the values after it stay as they were. Every node that seeds the same path reads the same value from it, wherever it sits in an expression. The chanterelle template gives every specimen one age this way, read by the stipe, the cap and the nodes that pose it. `select`, `boolean` and `seed` inputs and `integer` inputs set by a setting (the resolutions) are never expressions, `int` reads them.

`branch` and `instance` set `ctx.alpha` before they evaluate the inputs of each branch or instance, for example its `length` or `size`: alpha is where it sits on its parent path. `stem` and `branch` read `thickness` with `ctx.float_along` at every point of the path they create, there alpha goes along that path. `float_along` draws random values once, so a `random` thickness stays the same along the path. The path is then multiplied by `1 - alpha * taper`: `taper` 1 (the default) makes a cone, 0 leaves the profile to the thickness input. Everywhere else alpha is 0. The second word of the alpha expression keeps it from being read as a plain value.

Alpha is only one number that changes from place to place, so it can't tell a node about the path itself. `stem`, `branch`, `instance`, `gravity`, `rotate`, `noise`, `keep_apart` and `apply_force` also call `ctx.set_path` (or `ctx.set_path_info` for a stem that doesn't exist yet) before they read the inputs of each path. It sets `ctx.path`, what the `path_info` node reads: the `index` in the header of the path (the number among the paths its creator made on the same parent, it stays when other paths come or go), the place of the path among the paths the node works on (the `position`) and how many there are, its depth, where on its parent it grows (the `parent_alpha` of its link, 0 for a stem) and its length. `set_path` also sets alpha to the position of the path, from 0 at the first to 1 at the last, so `alpha` reads the same as `path_info` with position until a node sets it to something else. A `branch` or `instance` works on the path it places its branches or instances on, so a `branch` on the points of a whorl reads where the whorl sits on its stem. A `path_info` value doesn't change along a path or from one branch to the next, only from path to path.

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

`stem` makes paths without a parent, `branch` makes every new branch with the path it grows on as parent. `join` keeps the ids as they are, only a list that clashes with one joined before it (the same list joined twice) gets new ids, parents inside it follow. A node that removes a path leaves the links of its children pointing at nothing, nodes that follow a parent must ignore a `parent_id` they can't find. A node that shortens a path changes what `parent_alpha` of its children means. A node that copies a path keeps its header, so the ids survive `gravity`, `rotate`, `noise`, `snap`, `avoid`, `keep_apart`, `neighbour_sensing` and `apply_force`.

## Force

```
[3, kind, x, y, z, strength, radius, falloff]    all but the first two words are f32
```

A `force` output is a list of these, so a later node can combine forces. `kind` 0 is a point force: it pulls towards `x, y, z`, a negative `strength` pushes away. It reaches `radius` far, `falloff` 0 keeps the full strength up to the radius and 1 fades it smoothly to nothing at the radius. `Force::sample` in `nodarium_utils` gives the push at a position, `decode_forces` and `sample_forces` read a list of them. `apply_force` bends paths with them: the base of a path stays, the segments keep their length. Paths follow the spot on their parent they grow on, so a bent stem carries its branches. `PathStructure` and `follow_parents` in `nodarium_utils::geometry` do that for any node that moves paths.

How far a segment turns towards the push at its middle is `strength * t^(0.5 + 4 * stiffness)`. The `weight` input decides what `t` is:

- **along each path** (the default): `t` is the place of the segment along its own path, 0 at the base and 1 at the tip, and `strength` is read with alpha going along the path. Two paths at the same place bend by different amounts when they differ in length or start, so paths that lie on top of each other drift apart.
- **by place**: `t = min(1, d / reach)`, where `d` is the straight distance of the middle of the segment from the root of the path in metres, and `strength` is read with alpha = `t`. Every path that passes through a place gets the same weight there. `reach` 0 gives every segment the full weight, only the falloff of the forces is left.

The root is where a path grows from: the point of the `root` input that sits on its parent (its `parent_id` is the path's parent, the one with the closest alpha if there are several), else the spot on its parent in the list, else its own base. The `root` input is for paths whose parent is not in the list, like the rays of a `cap`, which get the cap's `hinge`. Without it each ray grows from its own base and the rays a cap adds further out bend on their own. A path that starts away from its root gets a straight lead-in from the root to its base, in steps about as long as its own segments. The lead-in is bent with it and dropped, so the base moves as far as a path from the root straight through the base would move there. A bend only ever turns a segment away from its own rest direction, so turns don't carry over from one segment to the next. A path that is the outer part of a longer path that runs straight from the root is therefore bent exactly like that part of it. Where the longer path curves on its way to the base, the base is off by how differently the lead-in and that way bend, mostly where the bend is strong close to the root: keep a `reach` there, with `reach` 0 such bases move far. `gravity` in chain mode has the same `weight`, `reach` and `root`: by place it turns a segment by `strength * 90° * t^(0.5 + 4 * elasticity)`, and a path moves with the spot on its parent first, like in `apply_force`. The closed-form mode ignores them. `place_root` and `PlaceChain` in `nodarium_utils::geometry` implement this for any node that bends paths.

## Points

```
[4, parent_id, depth, index, x, y, z, dx, dy, dz, tx, ty, tz, scale, alpha]    all but the first four words are f32
```

A `points` output is a list of these: spots on paths where a later node can grow or place something. `parent_id` is the `id` of the path the point sits on, `alpha` where on it (0 to 1, along its length, like `parent_alpha`), `depth` the depth of that path and `scale` its thickness there. `tangent` (`tx, ty, tz`) is the direction of the path at the point, `direction` (`dx, dy, dz`) is orthogonal to it and points away from the path: a branch grows that way, an instance sticks out that way, before their own angle tilts them. `index` is the number of the point among the points its node made on the same path, what grows on a point uses it as its own index, so its id stays the same when other points come or go. `Point`, `encode_points`, `decode_points` and `group_points_by_parent` in `nodarium_utils` read and write them.

`trunk_kink` bends paths at a few spots: everything above a kink turns by `angle`, away from the direction of the kink, and the kinks are turned around the path by `rotation` from one to the next (180 bends left and right in turn). The side the kinks turn from is carried along with every bend, so kinks 180 degrees apart stay in one plane. A kink between two points of a path adds a point there, the path keeps its length and its header. It gives the bent paths and a point on every kink, its direction points to the outer side of the bend and its tangent lies halfway between the path below and above it. Paths that grow on a bent path move with the spot they are attached to (`follow_parents`), they are moved but not turned.

`keep_apart` moves whole paths without a parent sideways (in x and z), so a circle of `radius` around each of them, measured `at` a place along the path (0 base, 1 tip), stays clear of the others. It reads `radius` with `ctx.set_path` for every path, so `path_info` length lets the room follow the size of each path. Overlapping pairs are pushed apart for a few `iterations`, the larger circle moves less (by the share of the other's area). Paths that grow on a moved path follow it (`follow_parents`). Because a `random` draws the same value for a path in every node that seeds it with that path, `keep_apart` and a `cap` that read the same expression first get the same radius.

`points_on_path` spreads `amount` points along every path of the `depth` deepest layers, from `lowest_point` to `highest_point` like the branches of `branch`. Each point is turned around its path by `rotation` from the one before it, and `offset_single` moves every second point up toward the next one, 1 makes pairs. Every point reads these inputs itself after `ctx.set_path` and `ctx.alpha`, so a random node gives each point its own turn. The points carry the id of their path, so `branch_from_points` and `instance_from_points` can grow on the same spots.

`random_break` breaks off some paths at a random spot between `lowest_break` and `highest_break`, `chance` is how likely a path breaks. Whether a path breaks and where only depend on the seed and the path id, so the same paths stay broken when other paths come or go. A broken path is resampled up to the break and keeps its number of points. Paths that grew above the break are removed, with everything growing on them, the ones below get their `parent_alpha` rescaled to the shorter parent, so they stay where they were.

`fill_gaps` adds paths between neighbouring paths of a fan, where they drift more than `spacing` path thicknesses apart, like the rays of a Myzel cap towards its margin. A fan is the paths one node made on one parent. Neighbours are ordered by the angle of their tips around the axis of the fan, the normal of the plane the tips spread in, and the last and the first are neighbours when the fan goes all the way round. A new path starts where the gap opens, at the same share of the length of both neighbours, and turns around the axis with them, so it keeps their distance from the axis. It sits halfway between them, moved by up to 15% of the gap by the seed, and grows from half to its full thickness over its first two radii. Every gap is filled before the gaps between the new paths, up to 1024 paths in a fan. A new path is a child of the same parent made by the same node as its neighbours, with the next free index, so `separate` keeps it with them.

`separate` takes apart paths that were joined: the first output has the paths made by the nodes that made the paths in `like`, the second the rest.

`branch_from_points` grows a branch from every point of a `points` input, for example the points of `points_on_path` or the kinks of `trunk_kink`. A branch grows along the direction of its point, `angle` tilts it toward the tangent (positive is up), its thickness starts at `thickness` times the thickness at the point and `taper` thins it to the tip. The id is `PathLink::child(creator, point.parent_id, point.alpha, point.index)`, so a branch keeps its id when other points come or go, and its depth is one below the point. The points are grouped by their path and `ctx.set_path` runs once per path, so random values don't start over at every branch. A point whose path is not in `plant` still grows its branch. Like in `branch`, a branch as long as the longest path gets `resolution_curve` points, shorter ones get fewer, and the branches of a path come right before it.

`instance` takes one geometry or a list of them from a `join` node, and every instance places one of them. With `pick` random (the default) the choice is `path_id(seed, path id, instance index)`, so it only depends on the seed, the path and the number of the instance. `bias` (hidden, -1 to 1) skews it: above 0 the later geometries of the list are picked more often, below 0 the earlier ones. At 1 the last of three geometries gets about 80% of the instances and the first about 1%, at 0 all are picked equally often. With `pick` index the `index` input decides: 0 to 1 spans the list evenly, 0 is the first geometry and 1 the last, values outside are clamped. `index` and `bias` are read for every instance after `ctx.alpha` is set, like `size`, so `alpha` into `index` goes through the list from the base of the path to its tip, `path_info` with `parent position` picks by the height or age of the path, and `alpha` into `bias` makes the random pick change along the path. Instances scaled to nothing are dropped after the choice, so they don't move the picks of the others.

`instance_from_points` puts a copy of `geometry` on every point of a `points` input, for example a leaf on the points of `points_on_path` or on the kinks of `trunk_kink`. Like in `instance`, the geometry grows along its -z axis and its face looks along +y. It sticks out along the direction of the point, `angle` tilts it toward the tangent (positive toward the tip), and it faces up to the light as far as it can. The size is `(point.scale + 0.01) * size`, and the path id of every instance is `point.parent_id`. The points are grouped by their path, `ctx.set_path` runs once per path and `ctx.alpha` is the alpha of the point.

`umbel` grows an umbel at the tip of every path of the `layers` deepest layers (1 by default): `amount` straight rays that start at the tip, like the flower heads of wild carrot, fennel or elder. The axis of the umbel is the direction of the last segment of the path. Ray i of n sits `sqrt((i + 1) / n)` of the way out, so the rays fill the disc evenly and the last one is the outer ray, `length` long and `opening` degrees from the axis. They are turned around the axis by the golden angle, plus a jitter of up to a quarter of the spacing of n even rays to either side. `shape` sets where the tips lie: 0 puts them on a flat top as high as the tip of the outer ray, so the inner rays are shorter, 1 on a dome where every ray is `length` long, values in between blend the two and negative ones make a cup. `length_variation` makes every ray up to that part shorter. The jitter and the variation only depend on `seed` and the id of the ray. With `depth` 2 every ray carries an umbellet, an umbel `umbellet_scale` times as long and as thick, so a compound umbel has `amount²` tips, depth 3 nests once more. An umbellet faces along the axis of its umbel on a flat top and along its ray on a dome. `length`, `opening`, `shape` and the other inputs are read once per umbel after `ctx.set_path` and with alpha 1, the `thickness` (absolute, in meters) with `ctx.float_along` along every ray. A ray is `PathLink::child(creator, parent, 1, i)` of the path or ray it grows on and one layer deeper, so `gravity`, `noise` and the `variants` of `to_geometry` treat it like a branch, and the rays of a path come right before it. The second output is a point at the tip of every ray of the last level, with the ray as its path, alpha 1, the ray direction as tangent and a direction that points away from the axis. `instance_from_points` with `angle` 90 places a flower along the ray on each of them, an `instance` with `lowest_instance` and `highest_instance` 1 puts a ring of petals around every tip.

`to_geometry` bakes everything it gets into one geometry, so a whole structure, for example an umbel with its flowers, can be placed by `instance` like a leaf. Paths are extruded like in `output` (with the `resolution.circle` setting), every instance becomes a transformed copy of its geometry, and geometry is added as it is. A baked part is usually small next to the plant, so `max_circle_segments` and `max_points` limit the segments around and the points along each path (0 is no limit), a path with more points is resampled evenly along its length. Instances scaled to nothing are left out, here and in `instance`. The geometry keeps the path id when all of its parts come from the same path, otherwise it is `NO_PATH`.

With `variants` on, `to_geometry` bakes every stem into its own geometry and gives them as a list, like `join` does, so `instance` picks one of them for every instance. A stem is a path of the input whose parent is not in the input. The paths that grow on it (found through `parent_id`), the instances whose path id is one of these paths and the geometry made from them go into its geometry, a part that belongs to no stem goes into every geometry. Every stem draws its own random values, so a `stem` with `amount` 5 and random inputs gives five different parts. With a single stem, or none, the result is one geometry as without `variants`.

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
