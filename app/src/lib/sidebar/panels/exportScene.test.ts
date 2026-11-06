import { BufferGeometry, Float32BufferAttribute, Group, Mesh, MeshBasicMaterial } from 'three';
import { describe, expect, it } from 'vitest';
import { createExportScene } from './exportScene';

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
