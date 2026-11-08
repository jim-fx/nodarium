import { BufferGeometry, Float32BufferAttribute, Group, Mesh, MeshBasicMaterial } from 'three';
import { GLTFExporter } from 'three/addons/exporters/GLTFExporter.js';
import { OBJExporter } from 'three/addons/exporters/OBJExporter.js';
import { describe, expect, it, vi } from 'vitest';
import { addObjVertexColors, createExportScene } from './exportScene';

// two triangles sharing an edge, the second one is gill
function twoGroups(groups = [1, 1, 1, 3], tissueGroups = true) {
  const geometry = new BufferGeometry();
  geometry.setAttribute(
    'position',
    new Float32BufferAttribute([0, 0, 0, 1, 0, 0, 0, 1, 0, 1, 1, 0], 3)
  );
  geometry.setAttribute('group', new Float32BufferAttribute(groups, 1));
  geometry.setIndex([0, 1, 2, 3, 2, 1]);
  const mesh = new Mesh(geometry, new MeshBasicMaterial());
  mesh.userData.myzel = true;
  mesh.userData.tissueGroups = tissueGroups;
  const scene = new Group();
  scene.add(mesh);
  return scene;
}

describe('createExportScene', () => {
  it('splits a mesh into one named mesh and material per group', () => {
    const [split] = createExportScene(twoGroups()).children;
    const parts = split.children as Mesh[];
    expect(parts.map((p) => p.name)).toEqual(['stipe', 'gill']);
    expect(parts.map((p) => (p.material as MeshBasicMaterial).name)).toEqual(['stipe', 'gill']);
    expect(parts.map((p) => p.geometry.getAttribute('position').count)).toEqual([3, 3]);
    expect([...parts[1].geometry.getAttribute('position').array]).toEqual([
      1,
      1,
      0,
      0,
      1,
      0,
      1,
      0,
      0
    ]);
  });

  it('numbers the groups of meshes made from paths', () => {
    const [split] = createExportScene(twoGroups([0, 0, 0, 1], false)).children;
    expect(split.children.map((p) => p.name)).toEqual(['group 0', 'group 1']);
  });

  it('keeps meshes of a single group as they are', () => {
    const scene = twoGroups([0, 0, 0, 0]);
    const [copy] = createExportScene(scene).children as Mesh[];
    expect(copy.geometry).toBe((scene.children[0] as Mesh).geometry);
  });
});

// linear red and blue, the last vertex has no color of its own
function colored() {
  const scene = twoGroups();
  const geometry = (scene.children[0] as Mesh).geometry;
  geometry.setAttribute(
    'color',
    new Float32BufferAttribute([1, 0, 0, 1, 1, 0, 0, 1, 0, 0, 1, 1, 1, 1, 1, 0], 4)
  );
  return scene;
}

describe('the colors survive export', () => {
  it('as vertex colors in an obj', () => {
    const exportScene = createExportScene(colored());
    const obj = addObjVertexColors(new OBJExporter().parse(exportScene), exportScene);
    const vertices = obj.split('\n').filter((l) => l.startsWith('v '));
    expect(vertices).toEqual([
      'v 0 0 0 1 0 0',
      'v 1 0 0 1 0 0',
      'v 0 1 0 0 0 1',
      'v 1 1 0 1 1 1',
      'v 0 1 0 0 0 1',
      'v 1 0 0 1 0 0'
    ]);
    // meshes without colors stay as the exporter wrote them
    const plain = createExportScene(twoGroups());
    const text = new OBJExporter().parse(plain);
    expect(addObjVertexColors(text, plain)).toBe(text);
  });

  it('as COLOR_0 in a glb', async () => {
    // the exporter reads its blobs with the browser's FileReader
    vi.stubGlobal(
      'FileReader',
      class {
        result: ArrayBuffer | null = null;
        onloadend = () => {};
        readAsArrayBuffer(blob: Blob) {
          blob.arrayBuffer().then((buffer) => {
            this.result = buffer;
            this.onloadend();
          });
        }
      }
    );
    const glb = await new GLTFExporter().parseAsync(createExportScene(colored()), {
      binary: true
    }) as ArrayBuffer;
    const jsonLength = new DataView(glb).getUint32(12, true);
    const json = JSON.parse(new TextDecoder().decode(new Uint8Array(glb, 20, jsonLength)));
    const primitives = json.meshes.flatMap((m: { primitives: object[] }) => m.primitives);
    expect(primitives).toHaveLength(2);
    for (const p of primitives) {
      const accessor = json.accessors[p.attributes.COLOR_0];
      expect(accessor.type).toBe('VEC4');
      expect(accessor.count).toBe(3);
    }
  });
});
