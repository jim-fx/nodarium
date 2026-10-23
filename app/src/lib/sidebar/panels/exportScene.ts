import { Group, InstancedMesh, Matrix4, Mesh, type Object3D } from 'three';

/**
 * Builds a copy of the scene that is safe to hand to the exporters:
 * hidden objects (the geometry pools keep stale meshes around) are dropped
 * and instanced meshes are expanded to one plain mesh per instance, since
 * the OBJ exporter ignores the instance matrices.
 */
export function createExportScene(source: Object3D): Group {
  const result = new Group();
  const matrix = new Matrix4();

  function visit(object: Object3D, parent: Object3D) {
    if (!object.visible) return;

    if (object instanceof InstancedMesh) {
      for (let i = 0; i < object.count; i++) {
        object.getMatrixAt(i, matrix);
        const mesh = new Mesh(object.geometry, object.material);
        matrix.decompose(mesh.position, mesh.quaternion, mesh.scale);
        parent.add(mesh);
      }
      return;
    }

    let copy: Object3D;
    if (object instanceof Mesh) {
      copy = new Mesh(object.geometry, object.material);
    } else {
      copy = new Group();
    }
    copy.name = object.name;
    copy.position.copy(object.position);
    copy.quaternion.copy(object.quaternion);
    copy.scale.copy(object.scale);
    parent.add(copy);

    for (const child of object.children) visit(child, copy);
  }

  for (const child of source.children) visit(child, result);
  result.position.copy(source.position);
  result.quaternion.copy(source.quaternion);
  result.scale.copy(source.scale);
  return result;
}
