# Plantarium

Plantarium is the node system in `nodes/max/plantarium`. Its nodes generate plants. All of them use `nodarium_utils` (`packages/utils`), which implements the formats below, so node code rarely touches them directly.

## Numbers

A `float` input is either a plain value or an **expression** produced by the `math`, `random`, `alpha`, `curve`, `path_info` or `wave` node:

```
[0, op, a, b]        math: a + b, a - b, a * b, a / b  (op 0-3)
[1, min, max, seed, uniform, per_path, salt, once]
                     random value between min and max, the last four are optional
[2, 0]               alpha: 0 at the base to 1 at the tip, see below
[2, 1]               alpha across: 0 on the middle of a geometry to 1 at its edge, see Colors
[2, 2]               alpha vein: 0 on a vein of leaf_surface to 1 at the edge of its groove, see Leaves
[3, value, min, max, x0, y0, x1, y1, ...]
                     curve: min + curve(value) * (max - min), value clamped to 0-1
[5, field]           path_info: a value of the path the node works on, field is the option of the node:
                     0 position, 1 index, 2 count, 3 depth, 4 parent position, 5 length
[8, value, count, shape]
                     wave: a profile repeated count times from value 0 to 1, 0 on a crest and 1 in a trough,
                     shape 0 sine, 1 tooth, 2 saw, 3 round
```

An `integer` input that isn't set by a setting is a plain value or the expression of the `random_int` or `round` node:

```
[4, min, max, seed]  random_int: a whole number from min to max, both included, every one equally likely
[6, value]           round: a float expression rounded to the nearest whole number
```

`a`, `b`, `min`, `max` and `value` can be expressions themselves. The points of a curve are plain values, sorted by x from 0 to 1. Between them the curve is a monotone cubic, so it never overshoots a point. Nodes read these inputs with a `Ctx`, created once per execution: `ctx.float`, `ctx.vec3` and `ctx.int`. Calling them again gives a new random value, which is how a single `random` node gives every stem a different length, and a `random_int` node every branch a different number of leaves. A new `Ctx` starts the random values from the beginning, so a node gives the same result every time it runs. `set_path` (and `seed_path`, which `stem` calls for the stems it is about to make) restarts the random values from the `id` of the path, so a path draws the same values whether or not other paths come before it in the list. A `random` with `per_path` ("once per path") is the exception: its value only depends on the seed, the id of the path that was seeded last and the node that made it (`salt`, its creator id), and it draws nothing, so the values after it stay as they were. Every node that seeds the same path reads the same value from it, wherever it sits in an expression. The chanterelle template gives every specimen one age this way, read by the stipe, the cap and the nodes that pose it. A `random` with `once` ("once for the graph") draws its value for no path at all, so every node reads the same value, also the nodes that read it without a path, like `cracks` or `mix_color`. The single mushroom templates draw their size and shape this way, so the cap, the gills under it and the pattern on it agree. `select`, `boolean` and `seed` inputs and `integer` inputs set by a setting (the resolutions) are never expressions, `int` reads them.

`branch` and `instance` set `ctx.alpha` before they evaluate the inputs of each branch or instance, for example its `length` or `size`: alpha is where it sits on its parent path. `stem` and `branch` read `thickness` with `ctx.float_along` at every point of the path they create, there alpha goes along that path. `float_along` draws random values once, so a `random` thickness stays the same along the path. The path is then multiplied by `1 - alpha * taper`: `taper` 1 (the default) makes a cone, 0 leaves the profile to the thickness input. Everywhere else alpha is 0. The second word of the alpha expression keeps it from being read as a plain value.

`wave` shapes a value like `curve` does, alpha when nothing is connected, but repeats: the value is multiplied by `count`, and every whole step from one crest to the next goes through the same profile, 0 on the crest and 1 in the trough. `sine` undulates, `tooth` has straight flanks and sharp crests, and `round` has round crests and sharp troughs; these three start on a crest at value 0 and reach the trough halfway. `saw` starts in a trough, climbs slowly to the crest at the end of each step and drops back at once, so its teeth lean toward higher values. `count` is a float, so `math` or `random` can set it, and a count that isn't whole stops partway through a step at value 1. Multiplied with a depth and fed into the `margin` of `leaf_surface`, it cuts teeth into a leaf (`tooth` dentate, `saw` serrate, `round` crenate), a `curve` of alpha on top keeps the teeth off the base. Added to the thickness of a stem, it makes rings, like the nodes of bamboo or horsetail.

A color input holds the sRGB bytes `[0, 4, r, g, b, 1, 1]`, or the expression of the `mix_color` node `[7, a, b, factor]`, where `a` and `b` are colors and `factor` a float. `ctx.color` reads it like a float input, the factor with alpha and the path, see Colors.

Alpha is only one number that changes from place to place, so it can't tell a node about the path itself. `stem`, `branch`, `instance`, `gravity`, `rotate`, `noise`, `keep_apart` and `apply_force` also call `ctx.set_path` (or `ctx.set_path_info` for a stem that doesn't exist yet) before they read the inputs of each path. It sets `ctx.path`, what the `path_info` node reads: the `index` in the header of the path (the number among the paths its creator made on the same parent, it stays when other paths come or go), the place of the path among the paths the node works on (the `position`) and how many there are, its depth, where on its parent it grows (the `parent_alpha` of its link, 0 for a stem) and its length. `set_path` also sets alpha to the position of the path, from 0 at the first to 1 at the last, so `alpha` reads the same as `path_info` with position until a node sets it to something else. A `branch` or `instance` works on the path it places its branches or instances on, so a `branch` on the points of a whorl reads where the whorl sits on its stem. A `path_info` value doesn't change along a path or from one branch to the next, only from path to path.

## Path

```
[0, stem_depth, id, parent_id, parent_alpha, creator, index, ...points]    each point: x, y, z, thickness
[16, stem_depth, id, parent_id, parent_alpha, creator, index, ...points, ...colors]
```

A path with the type 16 (`COLORED_PATH`) has an RGBA8 color for every point after its points, see Colors. `is_path` and `wrap_path` take both, `wrap_path` gives the points and the colors apart, so a node that moves the points of a path keeps its colors without knowing them.

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

The root is where a path grows from: the point of the `root` input that sits on its parent (its `parent_id` is the path's parent, the one with the closest alpha if there are several), else the spot on its parent in the list, else its own base. The `root` input is for paths whose parent is not in the list, like the rays of a `cap` split off their stipe, which get the point the cap sits at (`points_on_path` at the stipe tip). Without it each ray grows from its own base and the rays a cap adds further out bend on their own. A path that starts away from its root gets a straight lead-in from the root to its base, in steps about as long as its own segments. The lead-in is bent with it and dropped, so the base moves as far as a path from the root straight through the base would move there. A bend only ever turns a segment away from its own rest direction, so turns don't carry over from one segment to the next. A path that is the outer part of a longer path that runs straight from the root is therefore bent exactly like that part of it. Where the longer path curves on its way to the base, the base is off by how differently the lead-in and that way bend, mostly where the bend is strong close to the root: keep a `reach` there, with `reach` 0 such bases move far. `gravity` in chain mode has the same `weight`, `reach` and `root`: by place it turns a segment by `strength * 90° * t^(0.5 + 4 * elasticity)`, and a path moves with the spot on its parent first, like in `apply_force`. The closed-form mode ignores them. `place_root` and `PlaceChain` in `nodarium_utils::geometry` implement this for any node that bends paths.

## Points

```
[4, parent_id, depth, index, x, y, z, dx, dy, dz, tx, ty, tz, scale, alpha]    all but the first four words are f32
```

A `points` output is a list of these: spots on paths where a later node can grow or place something. `parent_id` is the `id` of the path the point sits on, `alpha` where on it (0 to 1, along its length, like `parent_alpha`), `depth` the depth of that path and `scale` its thickness there. `tangent` (`tx, ty, tz`) is the direction of the path at the point, `direction` (`dx, dy, dz`) is orthogonal to it and points away from the path: a branch grows that way, an instance sticks out that way, before their own angle tilts them. `index` is the number of the point among the points its node made on the same path, what grows on a point uses it as its own index, so its id stays the same when other points come or go. `Point`, `encode_points`, `decode_points` and `group_points_by_parent` in `nodarium_utils` read and write them.

`trunk_kink` bends paths at a few spots: everything above a kink turns by `angle`, away from the direction of the kink, and the kinks are turned around the path by `rotation` from one to the next (180 bends left and right in turn). The side the kinks turn from is carried along with every bend, so kinks 180 degrees apart stay in one plane. A kink between two points of a path adds a point there, the path keeps its length and its header. It gives the bent paths and a point on every kink, its direction points to the outer side of the bend and its tangent lies halfway between the path below and above it. Paths that grow on a bent path move with the spot they are attached to (`follow_parents`), they are moved but not turned.

`stem` makes `amount` stems from its `origin` straight up. Given `points`, like the points `sample_mesh` spreads over a substrate, every stem starts on a point instead and grows along its direction, the stems take the points in turn: with as many points as stems every stem has its own, with fewer the stems of a point grow out of one spot. The origin is not used then. The ids and the random values of the stems are the same with points as without, so a stem keeps its length when the points come or go.

`keep_apart` moves whole paths without a parent sideways (in x and z), so a circle of `radius` around each of them, measured `at` a place along the path (0 base, 1 tip), stays clear of the others. With `lean` (hidden) the base of every path stays where it is and the path turns around it, so the place at `at` moves aside instead, for stems that start on a substrate, which a move would lift off it. It reads `radius` with `ctx.set_path` for every path, so `path_info` length lets the room follow the size of each path. Overlapping pairs are pushed apart for a few `iterations`, the larger circle moves less (by the share of the other's area). Paths that grow on a moved path follow it (`follow_parents`). Because a `random` draws the same value for a path in every node that seeds it with that path, `keep_apart` and a `cap` that read the same expression first get the same radius.

`points_on_path` spreads `amount` points along every path of the `depth` deepest layers, from `lowest_point` to `highest_point` like the branches of `branch`. Each point is turned around its path by `rotation` from the one before it, and `offset_single` moves every second point up toward the next one, 1 makes pairs. Every point reads these inputs itself after `ctx.set_path` and `ctx.alpha`, so a random node gives each point its own turn. The points carry the id of their path, so `branch_from_points` and `instance_from_points` can grow on the same spots.

`random_break` breaks off some paths at a random spot between `lowest_break` and `highest_break`, `chance` is how likely a path breaks. Whether a path breaks and where only depend on the seed and the path id, so the same paths stay broken when other paths come or go. A broken path is resampled up to the break and keeps its number of points. Paths that grew above the break are removed, with everything growing on them, the ones below get their `parent_alpha` rescaled to the shorter parent, so they stay where they were.

`fill_gaps` adds paths between neighbouring paths of a fan, where they drift more than `spacing` path thicknesses apart, like the rays of a Myzel cap towards its margin. A fan is the paths one node made on one parent. Neighbours are ordered by the angle of their tips around the axis of the fan, the normal of the plane the tips spread in, and the last and the first are neighbours when the fan goes all the way round. A new path starts where the gap opens, at the same share of the length of both neighbours, and turns around the axis with them, so it keeps their distance from the axis. It sits halfway between them, moved by up to 15% of the gap by the seed, and grows from half to its full thickness over its first two radii. Every gap is filled before the gaps between the new paths, up to 1024 paths in a fan. A new path is a child of the same parent made by the same node as its neighbours, with the next free index, so `separate` keeps it with them.

`separate` takes apart paths that were joined: the first output has the paths made by the nodes that made the paths in `like`, the second the rest.

`branch_from_points` grows a branch from every point of a `points` input, for example the points of `points_on_path` or the kinks of `trunk_kink`. A branch grows along the direction of its point, `angle` tilts it toward the tangent (positive is up), its thickness starts at `thickness` times the thickness at the point and `taper` thins it to the tip. `thickness` goes up to 3, so a branch can be thicker than its path, which side veins need. The id is `PathLink::child(creator, point.parent_id, point.alpha, point.index)`, so a branch keeps its id when other points come or go, and its depth is one below the point. The points are grouped by their path and `ctx.set_path` runs once per path, so random values don't start over at every branch. A point whose path is not in `plant` still grows its branch. Like in `branch`, a branch as long as the longest path gets `resolution_curve` points, shorter ones get fewer, and the branches of a path come right before it.

`instance` takes one geometry or a list of them from a `join` node, and every instance places one of them. With `pick` random (the default) the choice is `path_id(seed, path id, instance index)`, so it only depends on the seed, the path and the number of the instance. `bias` (hidden, -1 to 1) skews it: above 0 the later geometries of the list are picked more often, below 0 the earlier ones. At 1 the last of three geometries gets about 80% of the instances and the first about 1%, at 0 all are picked equally often. With `pick` index the `index` input decides: 0 to 1 spans the list evenly, 0 is the first geometry and 1 the last, values outside are clamped. `index` and `bias` are read for every instance after `ctx.alpha` is set, like `size`, so `alpha` into `index` goes through the list from the base of the path to its tip, `path_info` with `parent position` picks by the height or age of the path, and `alpha` into `bias` makes the random pick change along the path. Instances scaled to nothing are dropped after the choice, so they don't move the picks of the others.

`instance_from_points` puts a copy of `geometry` on every point of a `points` input, for example a leaf on the points of `points_on_path` or on the kinks of `trunk_kink`. Like in `instance`, the geometry grows along its -z axis and its face looks along +y. It sticks out along the direction of the point, `angle` tilts it toward the tangent (positive toward the tip), and it faces up to the light as far as it can. The size is `(point.scale + 0.01) * size`, and the path id of every instance is `point.parent_id`. The points are grouped by their path, `ctx.set_path` runs once per path and `ctx.alpha` is the alpha of the point. Given a list of geometries it picks one for every instance like `instance` does, with the same `pick`, `index` and `bias`; at random the choice is `path_id(seed, point.parent_id, point.index)`, so it stays when other points come or go. `pick_geometry` and `encode_instances` in `nodarium_utils::geometry` are shared by both nodes.

`umbel` grows an umbel at the tip of every path of the `layers` deepest layers (1 by default): `amount` straight rays that start at the tip, like the flower heads of wild carrot, fennel or elder. The axis of the umbel is the direction of the last segment of the path. Ray i of n sits `sqrt((i + 1) / n)` of the way out, so the rays fill the disc evenly and the last one is the outer ray, `length` long and `opening` degrees from the axis. They are turned around the axis by the golden angle, plus a jitter of up to a quarter of the spacing of n even rays to either side. `shape` sets where the tips lie: 0 puts them on a flat top as high as the tip of the outer ray, so the inner rays are shorter, 1 on a dome where every ray is `length` long, values in between blend the two and negative ones make a cup. `length_variation` makes every ray up to that part shorter. The jitter and the variation only depend on `seed` and the id of the ray. With `depth` 2 every ray carries an umbellet, an umbel `umbellet_scale` times as long and as thick, so a compound umbel has `amount²` tips, depth 3 nests once more. An umbellet faces along the axis of its umbel on a flat top and along its ray on a dome. `length`, `opening`, `shape` and the other inputs are read once per umbel after `ctx.set_path` and with alpha 1, the `thickness` (absolute, in meters) with `ctx.float_along` along every ray. A ray is `PathLink::child(creator, parent, 1, i)` of the path or ray it grows on and one layer deeper, so `gravity`, `noise` and the `variants` of `to_geometry` treat it like a branch, and the rays of a path come right before it. The second output is a point at the tip of every ray of the last level, with the ray as its path, alpha 1, the ray direction as tangent and a direction that points away from the axis. `instance_from_points` with `angle` 90 places a flower along the ray on each of them, an `instance` with `lowest_instance` and `highest_instance` 1 puts a ring of petals around every tip.

`to_geometry` bakes everything it gets into one geometry, so a whole structure, for example an umbel with its flowers, can be placed by `instance` like a leaf. Paths are extruded like in `output` (with the `resolution.circle` setting), every instance becomes a transformed copy of its geometry, and geometry is added as it is. A baked part is usually small next to the plant, so `max_circle_segments` and `max_points` limit the segments around and the points along each path (0 is no limit), a path with more points is resampled evenly along its length. Instances scaled to nothing are left out, here and in `instance`. The geometry keeps the path id when all of its parts come from the same path, otherwise it is `NO_PATH`.

With `variants` on, `to_geometry` bakes every stem into its own geometry and gives them as a list, like `join` does, so `instance` picks one of them for every instance. A stem is a path of the input whose parent is not in the input. The paths that grow on it (found through `parent_id`), the instances whose path id is one of these paths and the geometry made from them go into its geometry, a part that belongs to no stem goes into every geometry. Every stem draws its own random values, so a `stem` with `amount` 5 and random inputs gives five different parts. With a single stem, or none, the result is one geometry as without `variants`.

## Leaves

`leaf` draws a leaf from a `shape`. `leaf_surface` grows one from its veins instead, so lobes, sinuses and teeth come from where the veins go. The veins are an ordinary path tree, made with the nodes that make branches:

`stem` (the midrib) → `rotate` x 90° → `points_on_path` (`rotation` 180, `offset_single` for pairs) → `branch_from_points` (the side veins) → `leaf_surface`

The leaf lies in the horizontal plane: `points_on_path` turns its points from the direction `tangent × y`, which only lies in the leaf plane for a path that lies flat, so the midrib is turned flat before the side veins grow on it. `alpha` into a `curve` into the `length` of `branch_from_points` makes the outline (long side veins in the middle, short ones at the base and tip), and its `thickness` is how far the blade reaches around a vein.

The reach of a vein is its thickness, in metres like every thickness, and `branch_from_points` sets it relative to the vein it grows on, with values above 1 for side veins whose blade reaches farther than the one of the midrib. Absolute reaches would need a second way to set the thickness of a branch, and a factor on the thickness in `leaf_surface` would scale every vein alike and still leave side veins thicker than the midrib. So the reach is the ordinary thickness of a path, any node that sets thickness shapes it, and `branch_from_points` allows a `thickness` up to 3. A palmate leaf starts from several stems turned out from one base (`stem` with `amount` 5, a `rotate` around y with `spread`). `gravity` and `noise` on the veins bend the leaf: `gravity` on the midrib arches it, on the side veins it lets the margins droop, `noise` in y makes them wavy. Only the paths go into `leaf_surface`, the veins are never extruded.

`leaf_surface` finds the frame of the leaf from the main veins, the paths whose parent is not among the veins: the base is where they start, on average, and the leaf points the way they go, seen from above. It then builds the leaf in stages, each one its own module, so a stage can be swapped without touching the others:

1. **Blade:** every vein is a round cone as wide as its thickness, and the blade is their smooth union on a grid, `resolution` cells (the `resolution.leaf` setting) across the longer side of the leaf. `smooth` is the fillet between the veins: 0 leaves a lobe or a tooth at every vein, more fills the sinuses until the margin is entire. Gaps the veins close in on every side are filled. The grid reaches a fillet beyond the veins and has at most about 1024 cells along a side, so a small leaf with a wide fillet gets larger cells instead of being cut off.
2. **Margin:** the outline of the blade, with alpha going along it from the base (0) to the tip (1) on both sides, by arc length. The `margin` input is read there and moves the margin in by that share of how far the blade reaches around the veins next to it, negative values push it out: 0.2 cuts a fifth of the reach, 1 cuts in to the vein. The reach at a point of the margin is the one of the closest point of the vein that makes the blade there, blended over a fillet and a cell between veins, so it does not jump in a sinus. A thin lobe or a tapering tip thus gets small teeth and keeps its shape, and the teeth grow with the leaf. Plain 0 leaves the outline as the veins make it. `wave` times a depth cuts teeth, a `curve` of alpha on top keeps them off the base.
3. **Mesh:** a constrained Delaunay triangulation of the margin, with the veins as chains of edges, so bending, relief and colour follow the veins. The margin is the line where the grid crosses zero, simplified along its curvature so teeth keep their points. A vein that tapers to nothing ends in a spike thinner than a cell, which the grid loses; it is rebuilt from the thickness of the vein less the share of it the margin cuts, so the tip keeps its full length. A vein that branches from another starts on a vertex of its parent's chain. Refinement then splits every triangle with an angle under 22° or longer edges than the size allows: about 2.5 cells along the margin and at the ends of the veins, growing to 8 cells in the middle of the blade. So `resolution` sets both how closely the margin follows the veins and how fine the mesh is, and a beech has about 1100 triangles at 96. The triangles run from vein to vein, and a row of points on both sides of every chain, beside each of its points and the middle of each of its edges, ends the groove of `relief` and the colour of the vein one row beside it. A bank is left out where it would come closer to the margin or another chain than it lies from its own, so where a side vein leaves the midrib the corner between them is filled from farther away.
4. **Lift:** the heights of the veins above the leaf plane, blended by distance, bend the flat mesh, and `relief` (hidden) sinks the veins into the blade, negative values raise them. Veins no higher than a ten-thousandth of the leaf, as `rotate` leaves them from rounding, count as flat.

The geometry is placed like the one of `leaf`: it grows along -z from its base at the origin and faces +y, and at `size` 1 a vein 1 m long makes a leaf as long as the one of `leaf` at size 1 (100 units). It goes into `instance` and `instance_from_points` like `leaf` does, and `set_color` reads alpha across from the midrib at x = 0. Without veins the result is an empty list, so `instance` places nothing.

### Leaf variants

`instance` places one geometry everywhere, so a plant with one leaf has the same leaf everywhere. With `variants` N, `leaf_surface` makes up to N leaves from one vein graph and gives them as a list, like `join` does, so `instance` and `instance_from_points` pick one for every instance, at random or by `index`. The main veins, the paths whose parent is not among the veins, are dealt out to the leaves in turn, and every other vein goes with the main vein it grows from:

```
stem (amount 4, random length) → rotate x 90° → points_on_path → branch_from_points → leaf_surface (variants 4) → instance
```

makes four leaves of one midrib each. A palmate leaf of five main veins takes `amount` 15 for three leaves of five. Dealt in turn, every leaf gets main veins from all over a fan spread by `rotate` with `spread` or by `path_info` position, not one side of it, so its veins spread a little less evenly than five stems alone would. With fewer main veins than variants there are fewer leaves, a single leaf is one geometry as without `variants`, and `variants` 1 makes one leaf of all veins as before.

The leaves differ by what the vein graph already draws per path: `stem` seeds every stem with its id, so a `random` length differs from leaf to leaf, and `points_on_path` and `branch_from_points` call `ctx.set_path` with the midrib, so a `random` with `per_path` or a `random_int` amount there gives every leaf its own side veins. `leaf_surface` itself reads `size`, `smooth`, `relief` and `margin` for every leaf after `ctx.set_path` with its first main vein, the leaf's place among the leaves as rank: a `random` with `per_path` into the depth of the teeth makes some leaves entire and others toothed, `path_info` reads the main vein, and alpha goes from 0 at the first leaf to 1 at the last, so `alpha` into `size` makes them grow from one to the next. Each leaf is built in its own frame from its own main veins, so the stems can stand anywhere.

Building a leaf is the expensive part, placing it is cheap, so the build time grows with the number of variants and hardly with the number of instances. A `set_color` after `leaf_surface` colours every leaf of the list, and two of them joined give twice as many leaves to pick from without building more.

There is no step in the runtime that runs a part of the graph several times with other seeds or alphas: every node runs once per build, with the graph's seed. A node that wants several results from one input has to make them itself, from data that is already in the input. The paths are that data here, `to_geometry` with `variants` does the same with its stems, and a list of geometries is what `join` already gives, so no new socket type or hidden data is needed.

### Vein colour

The `color` input of `leaf_surface` colours the blade, usually the expression of a `mix_color`. It is read at every vertex the way `set_color` reads a leaf, alpha from the base to the tip and alpha across from the midrib to the margin, and in addition with **alpha vein** (the `alpha` node set to vein) and the path of the nearest vein. Alpha vein is the distance of the vertex to the nearest vein as a share of the width of its groove: 0 on the vein, 1 at the edge of the groove and beyond. The nearest vein is the one whose groove the vertex lies deepest in, and `ctx.set_path` is called with it before its vertices are read, so `path_info` (depth tells the midrib, depth 1 for a `stem`, from the side veins, depth 2) and a `random` per path read that vein. A pale midrib with fainter side veins:

```
alpha (vein) → curve (1 at 0, 0 from 0.35) ─┐
path_info (depth) → math (− 1) → curve (1 → 0.55) ─ math (×) → mix_color (factor)
mix_color (a: the blade, for example a mix of two greens by alpha across; b: pale) → leaf_surface (color)
```

The vertices on a vein are exactly on it and the row of banks beside it is at the edge of its groove, so with a curve that falls between 0 and the banks only the vertices of the chain get the colour of the vein and it fades out over one row of triangles. The line is as wide as the groove of `relief`, about 1.5 cells or 15% of the reach of the vein, whichever is more, and a finer `resolution.leaf` makes it thinner. Unconnected, the blade has no colours and keeps the viewer's material, like the leaf of `leaf`; a `set_color` after `leaf_surface` replaces the colours of the whole blade.

The other way would have been to colour the veins with `set_color` (paths carry colours, type 16) and have `leaf_surface` paint them onto the blade near them. The blade itself still needs a colour then, and the only node that could give it one, a `set_color` after `leaf_surface`, overwrites every vertex; letting `set_color` keep colours that are already there would change what it does to every coloured geometry. The vein distance can't be read by `set_color` either, because a geometry doesn't carry it, and putting it into the geometry would send data a link doesn't show. So the colour is read where the veins are known, in `leaf_surface`, with the generic `alpha`, `curve`, `path_info` and `mix_color` nodes shaping it.

### Deform

The heights of the veins arch a leaf of `leaf_surface` and let its margins droop, but they can't bend it much past 60°, roll its margins or twist it. `deform` does that to any geometry that is placed like a leaf, growing along -z from its base at the origin and facing +y: a leaf of `leaf_surface` or `leaf`, a petal, or a list of them, which it deforms one by one, like `set_color`. Its `mode` is bend, curl or twist and its `amount` is in turns, so 0.25 is a right angle:

- **bend** arches the geometry around the x axis, positive towards its face (+y), negative away from it, a constant amount into an arc whose tip has turned by the amount. Past 0.25 it becomes a hook.
- **curl** rolls both halves of the geometry around lines along it, a constant amount into arcs whose edges have turned by the amount: 0.5 rolls a flat leaf into a tube, a little makes the channel of a grass blade, a negative amount on the margin alone rolls it under like the margin of sage or rosemary.
- **twist** turns the geometry around the z axis, positive turns its edge at +x up, a constant amount by the amount at the tip.

The amount is read as how fast the geometry turns where it is read, not how far it has turned, so the deformation keeps the lengths and the width of a leaf and a curve shapes where it bends. Bend and twist read it along the geometry, with alpha from 0 at its base to 1 at the vertex farthest along -z, and add up how far it turned from the base. Curl reads it at every place along and across, with alpha across from 0 on the middle (x = 0) to 1 at the edge, and adds up how far the half turned from the middle, so a `curve` of across that stays 0 up to 0.7 rolls only the outer band. The width at a place along is the widest vertex there, so a tapering leaf curls the same at every width and its tip rolls as tightly as its base. A plain `random` is drawn once per geometry, like `set_color` draws it.

```
alpha (across) → curve (0 up to 0.7, then up to 1; max -2) → deform (curl)    a rolled-under margin
alpha → curve (0 at the base, 1 at the tip; max 0.5) → deform (twist)           a twist that grows towards the tip
leaf_surface → deform (curl) → deform (twist) → deform (bend) → instance
```

Each mode works in the frame of the straight geometry: curl rolls around lines along z and twist turns around the z axis. After a bend those lines are no longer the axis of the leaf, so curl and twist go before bend. Bend turns the height of a vertex with it, like a strip bent with its thickness, and curl adds it on top of the roll instead: a cupped or bent leaf is higher than the roll of a margin is wide, and turned with it, its vertices would fold over. So curl also works on a leaf of `leaf` with its own `y_curve` and `x_curve`. The normals are computed again from the faces, area weighted, on the side the old ones were turned to. Colours, the path id and the faces stay as they were. `deform` moves the vertices and adds none, so a leaf of `leaf` needs an `x_resolution` of 5 or more to roll, and an amount of 0 leaves the geometry exactly as it was.

The curve of `leaf` (`y_curve`, `x_curve`, `y_twist`) does the same for its own leaf, as radians that are read along it. `deform` is the generic way: it works on `leaf_surface` and every other geometry and its amount is shaped by the generic `alpha` and `curve` nodes, so `leaf_surface` needs no inputs for it. It has one `mode` and one `amount`, three modes in a chain where a leaf needs them, which keeps the order visible.

## Geometry

```
[1, vertex_amount, face_amount, path_id, ...faces, ...positions, ...normals, ...colors?]
```

Faces are three vertex indices each, positions and normals three floats each. A geometry with colors has an RGBA8 color for every vertex after its normals, its length tells whether it has them. `path_id` is the `id` of the path an extruded geometry was made from (`output` extrudes every path), `-1` (`NO_PATH`) for geometry that doesn't come from a path, like a leaf or a box. The viewer finds the header of the path (creator, parent) with this id.

## Instances

```
[2, vertex_amount, face_amount, instance_amount, stem_depth, ...faces, ...positions, ...normals, ...matrices, ...path_ids, ...colors?]
```

The colors of the geometry, one per vertex, come last when it has them. One 4x4 matrix and one `path_id` per instance, the `id` of the path the instance sits on (`-1` if none). `instance` writes the path each leaf is placed on, so leaves can be picked by the part of the plant they sit on like the paths themselves. `readPathLink`, `readGeometryPathId` and `readInstancePathIds` in `@nodarium/utils` read these.

## Colors

A color on a path, a geometry or instances is an RGBA8 word, sRGB with red in the lowest byte, like the colors of a Myzel mesh. Paths and geometry without colors are encoded as they were, the viewer shows them in its material, and so are vertices whose alpha is 0 (`NO_COLOR`).

`leaf_surface` reads a color input the same way at every vertex of the blade, with alpha vein besides, see Leaves.

`set_color` gives the paths of the `depth` deepest levels (1, the deepest, by default) a color at every point, and every geometry in its input a color at every vertex. It reads the `color` input for every path after `ctx.set_path`, with alpha going along the path from 0 at its base to 1 at its tip, so a `random` with `per_path` gives every path its own color, a `path_info` into the factor of a `mix_color` one by the age of the path, and a curve of alpha a gradient along it. A plain `random` is drawn once per path, like the thickness. On a geometry alpha goes from its base, where an instance places it, to the vertex farthest from it, measured without x, the width of a leaf, so a leaf that curls up or down still ends at its tip. Alpha `across` (the `alpha` node set to across) goes from the middle of the geometry, x = 0, to its edge at the same place along, like from the midrib of a leaf to its margin, everywhere else it is 0. A leaf therefore gets its own gradient, margin or spots, and several `set_color` nodes on the same leaf, joined, let an `instance` pick a color for every leaf, at random or by its `index`, for example by `alpha` or the age of the path it sits on.

`output` and `to_geometry` give the extruded geometry of a path with colors the color of its point at every ring, the cap the color of its tip. `instance` and `instance_from_points` keep the colors of their geometry, `to_geometry` bakes them, and the vertices of parts without colors get `NO_COLOR` there. `trunk_kink`, `random_break` and the resampling of `to_geometry` keep the colors where they were along the length of the path. Other nodes that make paths, like `branch`, `umbel` or `fill_gaps`, make them without colors, so a `set_color` goes after the node that makes a level of paths and the nodes that shape it.

`mix_color` of the Myzel nodes gives a plain color when both colors and the factor are plain, and the expression otherwise, so the node that reads the color evaluates it where it uses it. The Myzel nodes read it once, without a path and with alpha 0, as before.

## Lists

Nodes can return several of these at once. They are nested with the bracket encoding from `packages/utils/src/flatTree.ts`. The `output` node's result is what the app renders.
