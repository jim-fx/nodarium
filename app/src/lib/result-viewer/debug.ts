import { splitNestedArray } from '@nodarium/utils';
import {
  BufferGeometry,
  type Group,
  InstancedMesh,
  Line,
  LineBasicMaterial,
  Matrix4,
  MeshBasicMaterial,
  SphereGeometry,
  Vector3
} from 'three';

function writePath(scene: Group, data: Int32Array): Vector3[] {
  const positions: Vector3[] = [];
  const f32 = new Float32Array(data.buffer);

  for (let i = 2; i + 2 < f32.length; i += 4) {
    const vec = new Vector3(f32[i], f32[i + 1], f32[i + 2]);
    positions.push(vec);
  }

  // Path line
  if (positions.length >= 2) {
    const geometry = new BufferGeometry().setFromPoints(positions);
    const line = new Line(
      geometry,
      new LineBasicMaterial({ color: 0xff0000, depthTest: false })
    );
    scene.add(line);
  }

  // Instanced spheres at points
  if (positions.length > 0) {
    const sphereGeometry = new SphereGeometry(0.05, 8, 8); // keep low-poly
    const sphereMaterial = new MeshBasicMaterial({
      color: 0xff0000,
      depthTest: false
    });

    const spheres = new InstancedMesh(
      sphereGeometry,
      sphereMaterial,
      positions.length
    );

    const matrix = new Matrix4();
    for (let i = 0; i < positions.length; i++) {
      matrix.makeTranslation(
        positions[i].x,
        positions[i].y,
        positions[i].z
      );
      spheres.setMatrixAt(i, matrix);
    }

    spheres.instanceMatrix.needsUpdate = true;
    scene.add(spheres);
  }

  return positions;
}

function clearGroup(group: Group) {
  for (let i = group.children.length - 1; i >= 0; i--) {
    const child = group.children[i];
    group.remove(child);
    // optional but correct: free GPU memory
    // @ts-expect-error three.js runtime fields
    child.geometry?.dispose?.();
    // @ts-expect-error three.js runtime fields
    child.material?.dispose?.();
  }
}

export function updateDebugScene(
  group: Group,
  data: Record<number, { type: string; data: Int32Array }>
) {
  clearGroup(group);
  return Object.entries(data || {}).map(([, d]) => {
    switch (d.type) {
      case 'path':
        splitNestedArray(d.data)
          .forEach(p => writePath(group, p));
    }
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    return (_g: Group) => {};
  }).flat();
}
