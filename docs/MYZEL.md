# Myzel

Myzel is the node system in `nodes/max/myzel`. Its nodes grow mushrooms. They work on the paths of [Plantarium](./PLANTARIUM.md): a stipe is a `stem`, the cap is rays that grow on it, strands fill both, and `hyphae_to_mesh` turns the strands into one body of tissue. The shared code is the crate `myzel_core` in `packages/myzel/src`, its module docs describe the mesh, lines, settings and color formats.

Myzel nodes pass `path`, `points`, `pattern`, `mesh` and `color` between them. A link carries exactly its type, there are no hidden items in a list of paths.

## The workflow

```
       time, or one age per mushroom, into every group

  Stipe ──stipe──▶ Cap ───rays───▶ Gills
   │ │              │ │              │
   │ │              │ └─margin─┐     │
   │ └─────stipe────┼───────▶ Veil   │
   │                │          │     │
 strands         strands      mesh  mesh
   │                │          │     │
   └─────▶ join ◀───┘          ▼     ▼
            │               join_meshes ──▶ output
            ▼                   ▲
     hyphae_to_mesh ───mesh─────┘
```

Every mushroom template is built from the same groups:

- **Stipe**: a `stem`, posed by Plantarium nodes, filled with `strands`. Gives the stipe as a path and its strands, colored.
- **Cap**: a `cap` at a point at the top of the stipe (`points_on_path` with one point at 1), posed by `fill_gaps`, `margin_waves`, `margin_curl` and `cap_open`, filled with its own `strands`. Gives the rays (with the stipe they grow on) and their strands, colored.
- **Gills**: a `reaction_diffusion` pattern under the rays, made into plates by `lamellae` or folds by `ridges`. Gives a mesh.
- **Veil** (only where the species has one): a `membrane` from the cap margin to a point on the stipe, and `skirt`s for the parts it tears into. Gives a mesh.

A group takes outcome inputs, like lengths, radii, colors and the time or the age, and gives paths or a mesh. At the top level the strands of stipe and cap are joined and go through `hyphae_to_mesh`, `join_meshes` joins that body with the gills and the veil, and `output` shows the mesh.

| template             | top level                                                                                                    | Stipe                                                                                                        | Cap                                                                  | Gills                                                  | Veil                                                             |
| -------------------- | ------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------- | ------------------------------------------------------ | ---------------------------------------------------------------- |
| `champignon-groomed` | `time`, the groups, `join`, `hyphae_to_mesh`, `join_meshes`, `output`                                        | time, lengths and radii of button and open stipe, color                                                      | stipe, time, radii and flesh of button and open cap, profile, colors | rays, reach, time, spacing, depth, young and old color | margin, stipe, time, ring height on button and open stipe, color |
| `chanterelle`        | one age per mushroom (`random` once per path), the groups, `join`, `hyphae_to_mesh`, `join_meshes`, `output` | age, young length, shortest and longest old length, color. Also gives the cap radius the troop is spaced for | stipe, age, radius, color                                            | rays, reach, spacing, depth, color                     | -                                                                |
| `oyster`             | the groups, `join`, `hyphae_to_mesh`, `join_meshes`, `output`                                                | length, radius, color                                                                                        | stipe, radius on the short and the long side, flesh, color           | rays, reach, spacing, depth, color                     | -                                                                |

## Nodes

Paths:

- `cap`: stipes (`plant`) and `points` → the stipes and the rays of a cap at every point. `radius`, `profile`, flesh `thickness`, number of `rays`.
- `fill_gaps` (Plantarium): rays → the rays and new ones where neighbours drift apart toward the margin.
- `margin_waves`: rays → rays whose margin waves up and down in lobes. `waves`, `amplitude`, `uneven_spacing`.
- `margin_curl`: rays → rays whose margin rolls in under the cap. `amount`.
- `cap_open`: rays → the cap closed to a button (0) or open as drawn (1). `opening`.
- `strands`: guide paths → a bundle of strands in the tube of every guide. `amount`, `thickness`, `spread`, `wobble`.
- `set_color`, `set_fibres`, `set_fill`: paths → the same paths with a color, a fibre relief or a fill for `hyphae_to_mesh`.

Points:

- `cap_margin`: rays → the margin of every cap as a ring of points, for `membrane` and `skirt`.

Pattern:

- `reaction_diffusion`: → stripes on a disc, every stripe a gill. `radius`, `inner_radius`, `spacing`, `alignment`, `irregularity`, `forking`, `steps`.

Meshes:

- `hyphae_to_mesh`: paths → one body of tissue around them. `fibres`, `fill`, `loose_strands`, hidden `blend`, the `resolution.field` setting.
- `lamellae`: rays and a pattern → thin plates with a sharp edge, free of the stipe or running down it. `depth`, `free_distance`, `decurrent`, `thickness`, `edge_wave`.
- `ridges`: rays and a pattern → low blunt folds, never deeper than wide, that run down the stipe. `depth`, `decurrent`, `thickness`, `edge_wave`.
- `membrane`: two attachments `a` and `b` (points or paths) → the closed skin between them. `thickness`, `sag`.
- `skirt`: one attachment → a skin that hangs from it to a torn edge. `length`, `flare`, `thickness`.
- `set_mesh_color`: mesh → the same mesh in one color.
- `join_meshes`: up to four meshes → one mesh, groups and colors kept.

Values and output:

- `time`: → the `time.t` setting of the graph, 0 to 1.
- `mix_color`: two colors and a `factor` → a color between them, for example from young to old.
- `output`: a mesh → what the app shows. Paths show as lines in the colors of their groups, a pattern as a disc, for debugging.

The Plantarium nodes the templates use besides `stem`: `trunk_kink`, `rotate` and `keep_apart` pose and space the stipes, `points_on_path` places the cap and the veil, `separate` takes the rays apart from the stipe so the cap gets its own `strands`, `twist`, `random_break`, `clump` and `neighbour_sensing` give the strands their grown look, and `curve`, `random`, `math`, `path_info`, `round`, `random_int` and `vec3` make the values.

## Conventions

These came out of the milestone "Myzel cleanup" (#212 to #224).

**One job per node.** A node does one thing and has at most 8 inputs. What another node can do moves out: the `twist` and the length variation of `strands` became the Plantarium `twist` and `random_break`, the lopsided cap became a `curve` into the cap radius, the torn parts of the veil became `skirt`. `set_color` colors paths, `set_mesh_color` meshes. `hyphae_to_mesh` meshes paths, `join_meshes` joins meshes, `output` only shows.

**Shape with `curve`, `random` and `alpha`.** A node has no inputs that shape another of its inputs (no `reach`, `irregularity`, `depth_curve` or `flare` curve). A `curve` into the input draws the profile, a `random` varies it, `alpha` (or a `curve` without a value) reads where the node is. Where a `curve` would only scale a `random`, the `random` goes into the `min` or `max` of the curve, so no `math` node is needed: the length of a chanterelle stipe is a `curve` of its age from 1.2 cm to a `random` between 6.4 and 8.6 cm.

**What is read where.** A node reads an input once for everything it stands for, and sets alpha and the path first, so a `random` or a `curve` varies it there:

| read                                                                                          | inputs                                                                                                                                               |
| --------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| per stipe (the stipe path is seeded, a `random` once per path gives every mushroom one value) | `cap.rays`; `cap_open.opening`; `margin_curl.amount`; `margin_waves.waves` and `uneven_spacing`; `lamellae` and `ridges` `decurrent` and `thickness` |
| per ray, alpha once around the axis (0.25 points where the stipe leans)                       | `cap.radius`                                                                                                                                         |
| along a ray, alpha 0 at the axis to 1 at the margin                                           | `cap.thickness`, `margin_curl.amount`, `margin_waves.amplitude`                                                                                      |
| per lobe                                                                                      | `margin_waves.amplitude`                                                                                                                             |
| along a gill, alpha 0 at the stipe to 1 at the margin, and per stipe                          | `lamellae.depth`, `ridges.depth`                                                                                                                     |
| per attachment                                                                                | `membrane.thickness` and `sag`; `skirt.thickness`, `skirt.length` around a path                                                                      |
| per point of a ring, alpha around the ring from its first point                               | `skirt.length` (0 leaves a gap)                                                                                                                      |
| along a skin, alpha 0 where it is attached to 1 at its edge; per point of a ring              | `skirt.flare`                                                                                                                                        |
| per strand, along it                                                                          | `strands.thickness`                                                                                                                                  |

**One age per mushroom.** A `random` with `per_path` draws one value for every path, the same in every node that reads it. The chanterelle draws one age per stipe at the top level and gives it to every group, so its stipe, the size of its cap, how far the cap opens and how much its margin curls and waves all follow the same age. The Champignon has one specimen and reads `time` instead.

**The cap grows at points.** `cap` takes the paths and the points to grow at, usually `points_on_path` with `amount` 1 and `lowest_point` = `highest_point` = 1 on the stipe. Its output is the stipes plus the rays, like `branch_from_points`. The pose nodes, `lamellae` and `ridges` take this one list and find the frame of every cap on the stipe the rays grow on (its place, axis and radius at the point). They bend only rays a `cap` made, every other path passes through, so the nodes can be chained in any order.

**`lamellae` or `ridges`.** True gills are `lamellae`: thin plates with a sharp edge, free of the stipe (`free_distance`, Agaricus) or running down it (`decurrent`, the oyster). False gills are `ridges`: low folds with a round back that run down the stipe, like the chanterelle. Both trace the stripes of a `reaction_diffusion` pattern.

**`membrane` and `skirt`.** `membrane` is the closed skin between two attachments, for example the margin from `cap_margin` and a point on the stipe from `points_on_path`. `skirt` hangs a skin from one attachment: around a point on a path it hangs out all around like a ring, from a ring of points it hangs on the inside like the fringe on a margin. On a ring its strands are spaced evenly along the points, so it follows a lopsided or wavy margin, and start inside the round end of the paths the points sit on (a ray of the cap, as thick as the point's scale), so the skin grows out of the flesh. A ring reads `length` and `flare` at every point: a `random` into them tears the fringe into flaps of different length and hang, a length of 0 leaves a gap. The Champignon switches from the membrane to two skirts when the veil tears, with curves of time into their thickness and length; its fringe gets a `random` length per point whose share of gaps grows with time, and a `random` into the `max` of its flare curve.

**Colors.** Paths get a color with `set_color`, which `hyphae_to_mesh` blends over the surface around them, meshes with `set_mesh_color`. A `mix_color` with a curve of time or age as its factor lets a color follow it, like the gills of the Champignon that darken.

**Groups.** A template groups its nodes into Stipe, Cap, Gills and Veil as above, so the top level reads as the workflow. Group inputs are the outcomes someone tunes, everything else stays inside.
