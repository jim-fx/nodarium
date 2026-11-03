import type { Graph } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import {
  matchTemplate,
  nodeSignatures,
  templateName,
  type TemplateSummary
} from './template-match';

type Nodes = Graph['nodes'];

const nodes = (...types: string[]): Nodes =>
  types.map((type, id) => ({ id, type: `max/plantarium/${type}`, position: [0, 0] })) as Nodes;

const knotweed = nodes(
  'output',
  'stem',
  'trunk_kink',
  'instance_from_points',
  'branch_from_points'
);
const fern = nodes('output', 'stem', 'branch', 'instance', 'gravity', 'noise');

const templates: TemplateSummary[] = [
  { name: 'devils-backbone', nodes: nodeSignatures({ nodes: nodes('output', 'stem') }) },
  { name: 'fern', nodes: nodeSignatures({ nodes: fern }) },
  { name: 'knotweed', nodes: nodeSignatures({ nodes: knotweed }) }
];

describe('templateName', () => {
  it('turns a project title into a template file name', () => {
    expect(templateName("Devil's Backbone")).toBe('devils-backbone');
    expect(templateName('  Lady Fern (copy) ')).toBe('lady-fern-copy');
  });
});

describe('matchTemplate', () => {
  it('picks the template named like the title', () => {
    const graph = { nodes: knotweed, meta: { title: "Devil's Backbone" } };
    expect(matchTemplate(graph, templates)).toBe('devils-backbone');
  });

  it('finds a renamed and edited project by the nodes it kept', () => {
    const edited = [...knotweed.slice(0, 4), ...nodes('gravity').map((n) => ({ ...n, id: 9 }))];
    const graph = { nodes: edited, meta: { title: 'Japanese Knotweed' } };
    expect(matchTemplate(graph, templates)).toBe('knotweed');
  });

  it('matches nothing when the graph shares too little with every template', () => {
    const graph = { nodes: nodes('output', 'box', 'triangle', 'math', 'vec3'), meta: {} };
    expect(matchTemplate(graph, templates)).toBeUndefined();
  });
});
