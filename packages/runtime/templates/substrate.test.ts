import { groupIdOf } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import { loadTemplate, substrates, withSubstrate } from './index';

const isSubstrate = (props?: Record<string, unknown>) => !!props && 'substrate' in props;

describe('withSubstrate', () => {
  for (const id of Object.keys(substrates)) {
    it(`adds one Substrate group to ${id}`, async () => {
      const template = await loadTemplate(id);
      const graph = await loadTemplate(id, { substrate: true });

      expect(template.nodes.some((n) => isSubstrate(n.props))).toBe(false);
      expect(graph.groups.length).toBe(template.groups.length + 1);
      expect(graph.groups.filter((g) => g.name === 'Substrate')).toHaveLength(1);

      const instances = graph.nodes.filter((n) => isSubstrate(n.props));
      expect(instances).toHaveLength(1);
      const instance = instances[0];
      expect(graph.groups.find((g) => g.id === groupIdOf(instance))?.name).toBe('Substrate');

      const out = graph.edges.filter(([from]) => from === instance.id);
      const mesh = out.filter(([, output]) => output === 0);
      expect(mesh).toHaveLength(1);
      expect(mesh[0][3]).toBe('d');
      expect(graph.nodes.find((n) => n.id === mesh[0][2])?.type).toBe('max/myzel/join_meshes');

      const places = out.filter(([, output]) => output === 1);
      expect(places.length).toBeGreaterThan(0);
      for (const [, , to, input] of places) {
        expect(input).toBe('points');
        const stipe = graph.nodes.find((n) => n.id === to);
        expect(graph.groups.find((g) => g.id === groupIdOf(stipe))?.name).toBe('Stipe');
      }
    });
  }

  it('leaves the template it is given as it is', async () => {
    const template = await loadTemplate('porcini');
    const before = structuredClone(template);
    withSubstrate(template, substrates.porcini!);
    expect(template).toEqual(before);
  });

  it('throws for a template without a substrate', async () => {
    await expect(loadTemplate('fern', { substrate: true })).rejects.toThrow('has no substrate');
  });

  it('throws for a graph without join_meshes', async () => {
    const fern = await loadTemplate('fern');
    expect(() => withSubstrate(fern, substrates.porcini!)).toThrow('join_meshes');
  });

  it('throws when the graph already has a substrate', async () => {
    const graph = await loadTemplate('porcini', { substrate: true });
    expect(() => withSubstrate(graph, substrates.porcini!)).toThrow('taken');
  });
});
