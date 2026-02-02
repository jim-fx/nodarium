import type { Graph } from '@nodarium/types';

export function grid(width: number, height: number) {
  const graph: Graph = {
    id: Math.floor(Math.random() * 100000),
    edges: [],
    nodes: []
  };

  const amount = width * height;

  for (let i = 0; i < amount; i++) {
    const x = i % width;
    const y = Math.floor(i / height);

    graph.nodes.push({
      id: i,
      position: [x * 30, y * 40],
      props: i == 0 ? { value: 0 } : { op_type: 0, a: 1, b: 0.05 },
      type: i == 0 ? 'max/plantarium/float' : 'max/plantarium/math'
    });

    graph.edges.push([i, 0, i + 1, i === amount - 1 ? 'input' : 'a']);
  }

  graph.nodes.push({
    id: amount,
    position: [width * 30, (height - 1) * 40],
    type: 'max/plantarium/output',
    props: {}
  });

  return graph;
}
