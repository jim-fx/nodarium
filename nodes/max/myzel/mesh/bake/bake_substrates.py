"""Bakes the substrates of the `mesh` node from Poly Haven CC0 assets, see SOURCES.md.

    python3 download_assets.py <assets>
    blender -b --factory-startup -P bake_substrates.py -- <assets> ../substrates

The stump is the scanned model, cropped to the stump and the ground around it, decimated and
scaled to real size. The ground patches are a grid on a ragged disc, raised by the displacement
map of a scanned ground texture. Every vertex gets the mean color of the diffuse map around it.

Binary format, little endian:
    u32 vertices, u32 faces, f32 min[3], f32 step[3]
    u16 position[3] per vertex, position = min + q * step, in metres, y up
    u8 sRGB color[3] per vertex
    u16 face[3] per face, counter-clockwise seen from outside
"""

import math
import os
import struct
import sys

import bmesh
import bpy
import numpy as np

STUMP_SCALE = 0.55
STUMP_CROP = 0.5
STUMP_FACES = 5000
STUMP_FALL = 0.035


def srgb_to_linear(c):
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def linear_to_srgb(c):
    c = np.clip(c, 0.0, 1.0)
    return np.where(c <= 0.0031308, c * 12.92, 1.055 * c ** (1 / 2.4) - 0.055)


def load_image(path, channels):
    """The pixels of an image, the first row at the bottom like Blender's uv"""
    image = bpy.data.images.load(path)
    image.colorspace_settings.name = "Non-Color"
    w, h = image.size
    pixels = np.empty(w * h * 4, dtype=np.float32)
    image.pixels.foreach_get(pixels)
    return pixels.reshape(h, w, 4)[:, :, :channels]


def box_down(pixels, k):
    h, w, c = pixels.shape
    return pixels[: h - h % k, : w - w % k].reshape(h // k, k, w // k, k, c).mean(axis=(1, 3))


def sample(pixels, u, v):
    """Bilinear, repeating"""
    h, w, _ = pixels.shape
    x, y = np.mod(u, 1.0) * w - 0.5, np.mod(v, 1.0) * h - 0.5
    x0, y0 = np.floor(x).astype(int), np.floor(y).astype(int)
    fx, fy = (x - x0)[:, None], (y - y0)[:, None]
    p = lambda yy, xx: pixels[np.mod(yy, h), np.mod(xx, w)]
    top = p(y0, x0) * (1 - fx) + p(y0, x0 + 1) * fx
    bottom = p(y0 + 1, x0) * (1 - fx) + p(y0 + 1, x0 + 1) * fx
    return top * (1 - fy) + bottom * fy


def write(path, positions, colors, faces):
    positions = np.asarray(positions, dtype=np.float64)
    assert len(positions) < 65536, len(positions)
    lo, hi = positions.min(axis=0), positions.max(axis=0)
    step = np.maximum(hi - lo, 1e-6) / 65535.0
    q = np.round((positions - lo) / step).astype("<u2")
    rgb = np.round(np.clip(colors, 0, 1) * 255).astype(np.uint8)
    with open(path, "wb") as f:
        f.write(struct.pack("<II", len(positions), len(faces)))
        f.write(struct.pack("<6f", *lo, *step))
        f.write(q.tobytes())
        f.write(rgb.tobytes())
        f.write(np.asarray(faces, dtype="<u2").tobytes())
    up = face_normals(positions, faces)[:, 1]
    print(f"{os.path.basename(path)}: {len(positions)} vertices, {len(faces)} faces, "
          f"{os.path.getsize(path)} bytes, size {np.round(hi - lo, 3)}, facing up {np.mean(up > 0):.2f}")


def face_normals(positions, faces):
    f = np.asarray(faces)
    a, b, c = positions[f[:, 0]], positions[f[:, 1]], positions[f[:, 2]]
    n = np.cross(b - a, c - a)
    return n / np.maximum(np.linalg.norm(n, axis=1, keepdims=True), 1e-12)


def stump(assets, out):
    folder = os.path.join(assets, "tree_stump_01")
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=os.path.join(folder, "model.gltf"))
    obj = next(o for o in bpy.context.scene.objects if o.type == "MESH")
    mesh = obj.data
    mesh.transform(obj.matrix_world)
    obj.matrix_world.identity()

    co = np.array([v.co for v in mesh.vertices])
    z = co[:, 2]
    centre = co[z > z.min() + 0.7 * (z.max() - z.min()), :2].mean(axis=0)

    bm = bmesh.new()
    bm.from_mesh(mesh)
    far = [f for f in bm.faces if np.linalg.norm(np.array(f.calc_center_median())[:2] - centre) > STUMP_CROP]
    bmesh.ops.delete(bm, geom=far, context="FACES")
    loose = [v for v in bm.verts if not v.link_faces]
    bmesh.ops.delete(bm, geom=loose, context="VERTS")
    # the importer splits the vertices at the seams of the uv map, decimated apart they open cracks
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
    bm.to_mesh(mesh)
    bm.free()

    decimate = obj.modifiers.new("decimate", "DECIMATE")
    decimate.ratio = STUMP_FACES / len(mesh.polygons)
    triangulate = obj.modifiers.new("triangulate", "TRIANGULATE")
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.modifier_apply(modifier=decimate.name)
    bpy.ops.object.modifier_apply(modifier=triangulate.name)

    diffuse = srgb_to_linear(box_down(load_image(os.path.join(folder, "textures", "tree_stump_01_diff_2k.jpg"), 3), 16))
    uv = mesh.uv_layers.active.data
    v = len(mesh.vertices)
    sums, counts = np.zeros((v, 3)), np.zeros(v)
    faces = []
    for poly in mesh.polygons:
        faces.append(list(poly.vertices))
        for li in poly.loop_indices:
            vi = mesh.loops[li].vertex_index
            u_, v_ = uv[li].uv
            sums[vi] += sample(diffuse, np.array([u_]), np.array([v_]))[0]
            counts[vi] += 1
    colors = linear_to_srgb(sums / np.maximum(counts, 1)[:, None])

    co = np.array([vert.co for vert in mesh.vertices])
    co[:, :2] -= centre
    # blender is z up, the nodes y up
    positions = np.stack([co[:, 0], co[:, 2], -co[:, 1]], axis=1) * STUMP_SCALE
    # the cut edge of the forest floor falls away, so nothing grows on it
    edge = np.hypot(positions[:, 0], positions[:, 2]) / (STUMP_CROP * STUMP_SCALE)
    fall = np.clip((edge - 0.72) / 0.28, 0, 1)
    positions[:, 1] -= STUMP_FALL * fall * fall
    boundary = boundary_vertices(faces)
    positions[:, 1] -= positions[boundary, 1].mean()
    write(os.path.join(out, "stump.bin"), positions, colors, faces)


def boundary_vertices(faces):
    edges = {}
    for f in faces:
        for a, b in ((f[0], f[1]), (f[1], f[2]), (f[2], f[0])):
            key = (min(a, b), max(a, b))
            edges[key] = edges.get(key, 0) + 1
    return sorted({i for e, n in edges.items() if n == 1 for i in e})


def patch(assets, out, name, texture, size, radius, spacing, dome, relief, centre_uv):
    """A ragged disc of ground: `size` is how many metres the texture spans, `dome` how high the
    middle stands over the rim and `relief` how deep the displacement map is"""
    folder = os.path.join(assets, texture)
    k = max(1, int(spacing / size * 2048 / 2))
    diffuse = srgb_to_linear(box_down(load_image(os.path.join(folder, "diffuse.jpg"), 3), k))
    disp = box_down(load_image(os.path.join(folder, "disp.jpg"), 1), k)

    # rings of vertices about `spacing` apart, stretched to a ragged outline
    rings = int(math.ceil(radius / spacing))
    starts, angles, ts = [], [], []
    for k in range(rings + 1):
        count = 1 if k == 0 else max(6, round(2 * math.pi * k))
        starts.append(len(ts))
        angles.extend(2 * math.pi * (np.arange(count) + 0.5 * (k % 2)) / count)
        ts.extend([k / rings] * count)
    starts.append(len(ts))
    angle, t = np.array(angles), np.array(ts)
    edge = radius * (1 + 0.07 * np.sin(3 * angle + 1.3) + 0.04 * np.sin(7 * angle + 0.4) + 0.02 * np.sin(13 * angle))
    x, z = t * edge * np.cos(angle), t * edge * np.sin(angle)

    u = centre_uv[0] + x / size
    v = centre_uv[1] - z / size
    rise = dome * np.clip(1 - t * t, 0, 1) ** 0.6
    smooth = lambda a, b, s: np.clip((s - a) / (b - a), 0, 1) ** 2 * (3 - 2 * np.clip((s - a) / (b - a), 0, 1))
    bumps = relief * (sample(disp, u, v)[:, 0] - 0.5) * (1 - smooth(0.75, 1.0, t))
    y = rise + bumps - 0.006 * smooth(0.85, 1.0, t)
    positions = np.stack([x, y, z], axis=1)
    colors = linear_to_srgb(sample(diffuse, u, v))

    faces = []
    for k in range(1, rings + 1):
        inner = list(range(starts[k - 1], starts[k]))
        outer = list(range(starts[k], starts[k + 1]))
        # zips the two rings, always stepping on the ring whose next vertex comes first around
        around = lambda ring, n: angle[ring[n % len(ring)]] + 2 * math.pi * (n // len(ring))
        i = j = 0
        while i < len(inner) or j < len(outer):
            a, b = inner[i % len(inner)], outer[j % len(outer)]
            next_inner = around(inner, i + 1) if len(inner) > 1 else math.inf
            next_outer = around(outer, j + 1)
            if j < len(outer) and (i >= len(inner) or next_outer <= next_inner):
                faces.append([a, outer[(j + 1) % len(outer)], b])
                j += 1
            else:
                faces.append([a, inner[(i + 1) % len(inner)], b])
                i += 1
    faces = [f for f in faces if len(set(f)) == 3]
    # counter-clockwise seen from above, so the faces look up
    faces = [f if face_normals(positions, [f])[0, 1] > 0 else [f[0], f[2], f[1]] for f in faces]
    positions[:, 1] -= positions[boundary_vertices(faces), 1].mean()
    write(os.path.join(out, name + ".bin"), positions, colors, faces)


def main():
    assets, out = sys.argv[sys.argv.index("--") + 1:][:2]
    os.makedirs(out, exist_ok=True)
    stump(assets, out)
    patch(assets, out, "moss", "forest_leaves_02", 3.0, 0.11, 0.0045, 0.035, 0.03, (0.18, 0.64))
    patch(assets, out, "needles", "forrest_ground_03", 2.0, 0.14, 0.0055, 0.012, 0.012, (0.5, 0.5))
    patch(assets, out, "leaves", "forest_leaves_04", 1.5, 0.14, 0.0055, 0.01, 0.02, (0.45, 0.55))


main()
