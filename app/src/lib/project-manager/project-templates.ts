import { defaultPlant, lottaFaces, plant, simple } from '$lib/graph-templates';
import type { Graph } from '@nodarium/types';

export type ProjectTemplate = {
  name: string;
  graph: Graph;
};

function toGraph(template: unknown): Graph {
  return { id: 0, groups: [], ...(template as Partial<Graph>) } as Graph;
}

export const projectTemplates: ProjectTemplate[] = [
  { name: 'Default Plant', graph: toGraph(defaultPlant) },
  { name: 'Plant', graph: toGraph(plant) },
  { name: 'Simple', graph: toGraph(simple) },
  { name: 'Lotta Faces', graph: toGraph(lottaFaces) }
];
