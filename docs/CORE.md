# Core

Core is the collection in `nodes/max/core`: the nodes every graph has, whichever system its other nodes come from. The value nodes give a number, a vector or a color, or an expression that the node reading it evaluates where it reads it. They take no paths or geometry, only the `output` takes geometry. Up to graph version 15 they lived in [Plantarium](./PLANTARIUM.md) (`time` and `mix_color` in [Myzel](./MYZEL.md)), the migration of the graph moves them.

- `float`: a number set on the node, to give the same value to several inputs.
- `math`: `a` and `b` added, subtracted, multiplied or divided.
- `random`: a value between `min` and `max`, drawn where it is read, or once per path, per instance or for the graph.
- `random_int`: a whole number from `min` to `max`.
- `round`: a float rounded to the nearest whole number.
- `curve`: a curve drawn on the node, read at `value` (alpha when nothing is connected) and scaled from `min` to `max`.
- `alpha`: where the node reading it is, 0 at the base to 1 at the tip, across a geometry or from a vein.
- `wave`: a profile repeated `count` times, like `curve` but periodic.
- `vec3`: three floats as a vector, with a gizmo in the viewer.
- `time`: the `time.t` setting of the graph, 0 to 1.
- `mix_color`: two colors and a `factor` → a color between them, for example from young to old.
- `output`: the end of every graph, shows the geometry and the instances that reach it and takes nothing else. Paths become geometry in a `to_geometry` before it. Up to graph version 17 Plantarium and Myzel had an output each, the migration puts the `to_geometry` in front of it where paths reached it, and sends a pattern to a debug node.

`path_info` stays in Plantarium, it reads the path a node works on.

## Expressions

A `float` input is either a plain value or an **expression** produced by the `math`, `random`, `alpha`, `curve`, `wave` or `path_info` node:

```
[0, op, a, b]        math: a + b, a - b, a * b, a / b  (op 0-3)
[1, min, max, seed, uniform, per_path, salt, once, per_instance]
                     random value between min and max, the last five are optional
[2, 0]               alpha: 0 at the base to 1 at the tip, see below
[2, 1]               alpha across: 0 on the middle of a geometry to 1 at its edge, see Colors in PLANTARIUM.md
[2, 2]               alpha vein: 0 on a vein of leaf_surface to 1 at the edge of its groove, see Leaves in PLANTARIUM.md
[3, value, min, max, x0, y0, x1, y1, ...]
                     curve: min + curve(value) * (max - min), value clamped to 0-1
[5, field]           path_info (Plantarium): a value of the path the node works on, field is the option of the node:
                     0 position, 1 index, 2 count, 3 depth, 4 parent position, 5 length, 6 grown, 7 age
[8, value, count, shape]
                     wave: a profile repeated count times from value 0 to 1, 0 on a crest and 1 in a trough,
                     shape 0 sine, 1 tooth, 2 saw, 3 round
[9, tag, value]      probe: the value, handed to the host with the tag as well, see below
```

An `integer` input that isn't set by a setting is a plain value or the expression of the `random_int` or `round` node:

```
[4, min, max, seed]  random_int: a whole number from min to max, both included, every one equally likely
[6, value]           round: a float expression rounded to the nearest whole number
```

`nodarium probe` (`packages/runtime/src/cli/probe.ts`) wraps the output of a node in a probe on every edge, with a tag per edge. `Ctx` evaluates the value as usual and hands it to the host import `nodarium_probe(tag, value, path id)`, which the runtime passes to the listener of `setProbeListener` in `@nodarium/utils`, and the runtime says which node runs at the time (`beforeNode` of the executor). So the command sees every value a node reads, on which path and through which edge, without a node knowing it is probed. A probe draws no random value, the plant stays the same. A plain value is not wrapped, some nodes take a single word for an unconnected input.

`a`, `b`, `min`, `max` and `value` can be expressions themselves. The points of a curve are plain values, sorted by x from 0 to 1. Between them the curve is a monotone cubic, so it never overshoots a point. Nodes read these inputs with a `Ctx` of `nodarium_utils` (`packages/utils`), created once per execution: `ctx.float`, `ctx.vec3` and `ctx.int`. Calling them again gives a new random value, which is how a single `random` node gives every stem a different length, and a `random_int` node every branch a different number of leaves. A new `Ctx` starts the random values from the beginning, so a node gives the same result every time it runs. `set_path` (and `seed_path`, which `stem` calls for the stems it is about to make) restarts the random values from the `id` of the path, so a path draws the same values whether or not other paths come before it in the list. A `random` with `per_path` ("once per path") is the exception: its value only depends on the seed, the id of the path that was seeded last and the node that made it (`salt`, its creator id), and it draws nothing, so the values after it stay as they were. Every node that seeds the same path reads the same value from it, wherever it sits in an expression. The chanterelle template gives every specimen one age this way, read by the stipe, the cap and the nodes that pose it. A `random` with `once` ("once for the graph") draws its value for no path at all, so every node reads the same value, also the nodes that read it without a path, like `cracks` or `mix_color`. The single mushroom templates draw their size and shape this way, so the cap, the gills under it and the pattern on it agree. A `random` with `per_instance` ("once per instance") draws its value for one instance: `instance` and `instance_from_points` call `ctx.set_instance` with the number of the instance on its path (the `index` of its point) before they read its inputs, so the `size` and the `angle` of a leaf read the same value and the next leaf another one. The id of an instance comes from the id of its path and that number, so its value stays when other paths come or go, and like `per_path` it draws nothing. `set_path` and `seed_path` end the instance. A node that places no instances reads it like `per_path`, and of the three options the coarser one wins: `once` over `per_path` over `per_instance`. `select`, `boolean` and `seed` inputs and `integer` inputs set by a setting (the resolutions) are never expressions, `int` reads them.

Where alpha and the path come from, the nodes that set them before they read their inputs, is described under Alpha and the path in [PLANTARIUM.md](./PLANTARIUM.md). Everywhere else alpha is 0. The second word of the alpha expression keeps it from being read as a plain value.

`wave` shapes a value like `curve` does, alpha when nothing is connected, but repeats: the value is multiplied by `count`, and every whole step from one crest to the next goes through the same profile, 0 on the crest and 1 in the trough. `sine` undulates, `tooth` has straight flanks and sharp crests, and `round` has round crests and sharp troughs; these three start on a crest at value 0 and reach the trough halfway. `saw` starts in a trough, climbs slowly to the crest at the end of each step and drops back at once, so its teeth lean toward higher values. `count` is a float, so `math` or `random` can set it, and a count that isn't whole stops partway through a step at value 1. Multiplied with a depth and fed into the `margin` of `leaf_surface`, it cuts teeth into a leaf (`tooth` dentate, `saw` serrate, `round` crenate), a `curve` of alpha on top keeps the teeth off the base. Added to the thickness of a stem, it makes rings, like the nodes of bamboo or horsetail.

## Colors

A color input holds the sRGB bytes `[0, 4, r, g, b, 1, 1]`, or the expression of the `mix_color` node `[7, a, b, factor]`, where `a` and `b` are colors and `factor` a float. `ctx.color` reads it like a float input, the factor with alpha and the path, see Colors in [PLANTARIUM.md](./PLANTARIUM.md). A factor that is an expression is read by the node that uses the color, Plantarium's `set_color` with alpha and the path, so a curve of alpha makes a gradient. Myzel nodes read it once, without a path.

## Time

`time` gives the `time.t` setting of the graph, 0 to 1, so a graph that has it grows with the setting, in the app and with `nodarium render --times`. The plant templates drive the hidden `growth` of their nodes with it, the mushroom templates give it to their groups.
