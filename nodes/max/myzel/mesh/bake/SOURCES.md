# Substrates

The substrates of the `mesh` node in `../substrates` are baked from scans of [Poly Haven](https://polyhaven.com). All of them are [CC0](https://creativecommons.org/publicdomain/zero/1.0/), public domain, no attribution needed. Only the baked meshes are in the repository, not the scans.

| substrate     | source                                                                                              | author     | licence | what the bake keeps                                                                                                                                  |
| ------------- | --------------------------------------------------------------------------------------------------- | ---------- | ------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `stump.bin`   | model [Tree Stump 01](https://polyhaven.com/a/tree_stump_01) (2k glTF, diffuse map)                 | Rob Tuytel | CC0     | the stump and the forest floor within 0.5 m of it, scaled by 0.55 to a stump about 33 cm across on a mound 56 cm across, decimated to 5000 triangles |
| `moss.bin`    | texture [Forest Leaves 02](https://polyhaven.com/a/forest_leaves_02) (2k diffuse and displacement)  | Rob Tuytel | CC0     | a cushion of moss 23 cm across, 3.5 cm high, from the mossy part of the scan around u 0.18, v 0.64, 3 cm of relief                                   |
| `needles.bin` | texture [Forest Ground 03](https://polyhaven.com/a/forrest_ground_03) (2k diffuse and displacement) | Rob Tuytel | CC0     | pine needle litter 29 cm across, 1.2 cm high, 1.2 cm of relief                                                                                       |
| `leaves.bin`  | texture [Forest Leaves 04](https://polyhaven.com/a/forest_leaves_04) (2k diffuse and displacement)  | Rob Tuytel | CC0     | leaf litter with twigs 29 cm across, 1 cm high, 2 cm of relief                                                                                       |

The ground patches are a mesh of rings about 5 mm apart on a ragged disc, raised by the displacement map, with the diffuse map averaged around every vertex as its color.

## Baking

With Blender 5.2 (any Blender with the glTF importer and numpy should do):

```sh
python3 download_assets.py /tmp/substrate-assets
blender -b --factory-startup -P bake_substrates.py -- /tmp/substrate-assets ../substrates
```

`bake_substrates.py` describes the binary format. The `mesh` node reads it with `include_bytes!` and computes the normals from the faces.
