import type { Graph, SerializedNode } from '@nodarium/types';

export function tree(depth: number): Graph {
  const nodes: SerializedNode[] = [
    {
      id: 0,
      type: 'max/plantarium/output',
      position: [0, 0]
    },
    {
      id: 1,
      type: 'max/plantarium/math',
      position: [-40, -10]
    }
  ];

  const edges: [number, number, number, string][] = [
    [1, 0, 0, 'input']
  ];

  for (let d = 0; d < depth; d++) {
    const amount = Math.pow(2, d);
    for (let i = 0; i < amount; i++) {
      const id0 = amount * 2 + i * 2;
      const id1 = amount * 2 + i * 2 + 1;

      const parent = Math.floor(id0 / 2);

      const x = -(d + 1) * 50 - 40;
      const y = i * 80 - amount * 35;

      nodes.push({
        id: id0,
        type: 'max/plantarium/math',
        position: [x, y]
      });
      edges.push([id0, 0, parent, 'a']);
      nodes.push({
        id: id1,
        type: 'max/plantarium/math',
        position: [x, y + 35]
      });
      edges.push([id1, 0, parent, 'b']);
    }
  }

  return {
    id: Math.floor(Math.random() * 100000),
    nodes,
    edges,
    groups: []
  };
}
