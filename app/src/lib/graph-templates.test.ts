import { CURRENT_GRAPH_VERSION } from '#lib/graph-migrations/index.ts';
import champignonGroomed from '#lib/graph-templates/champignon-groomed.json';
import chestnutTemplate from '#lib/graph-templates/chestnut.json';
import flyAgaricTemplate from '#lib/graph-templates/fly-agaric.json';
import { grid } from '#lib/graph-templates/grid.ts';
import porciniTemplate from '#lib/graph-templates/porcini.json';
import { tree } from '#lib/graph-templates/tree.ts';
import { projectTemplates } from '#lib/project-manager/project-templates.ts';
import type { Graph, NodeDefinition } from '@nodarium/types';
import { describe, expect, it } from 'vitest';

describe('graph-templates', () => {
  it('grid has an output node', () => {
    const result = grid(2, 2);
    expect(result.nodes.some(n => n.type === 'max/plantarium/output')).toBe(true);
  });

  it('tree has the output node as its root', () => {
    const result = tree(2);
    expect(result.nodes.find(n => n.type === 'max/plantarium/output')?.id).toBe(0);
  });

  it('tree grows with its depth', () => {
    expect(tree(0).nodes.length).toBeLessThan(tree(1).nodes.length);
    expect(tree(1).nodes.length).toBeLessThan(tree(2).nodes.length);
  });

  it('tree connects every node but the root once', () => {
    const result = tree(2);
    expect(result.edges.length).toBe(result.nodes.length - 1);
  });
});

describe('mushroom templates', () => {
  const groupsOf = (graph: Graph) => graph.groups.map((g) => g.name);

  it('the Chestnut mushroom is the Champignon with a Cuticle group', () => {
    const [champignon, chestnut] = [champignonGroomed, chestnutTemplate] as unknown as Graph[];
    expect(groupsOf(chestnut)).toEqual([...groupsOf(champignon), 'Cuticle']);
    const cuticle = chestnut.groups.find((g) => g.name === 'Cuticle')!;
    const types = cuticle.nodes.map((n) => n.type);
    expect(types.filter((t) => t === 'max/myzel/cracks')).toHaveLength(2);
    expect(types.filter((t) => t === 'max/myzel/cap_pattern')).toHaveLength(2);
    // between the body of hyphae_to_mesh and join_meshes
    const instance = chestnut.nodes.find((n) => n.props?.groupId === cuticle.id)!;
    const type = (id: number) => chestnut.nodes.find((n) => n.id === id)!.type;
    expect(chestnut.edges.filter((e) => e[2] === instance.id).map((e) => type(e[0]))).toContain(
      'max/myzel/hyphae_to_mesh'
    );
    expect(chestnut.edges.filter((e) => e[0] === instance.id).map((e) => type(e[2]))).toEqual([
      'max/myzel/join_meshes'
    ]);
  });

  it('lists the Chestnut mushroom with the mushrooms', () => {
    const names = projectTemplates.filter((t) => t.category === 'Mushrooms').map((t) => t.name);
    expect(names).toContain('Chestnut mushroom');
  });

  it('the Fly agaric has warts on the cuticle, a ring and a volva on its bulb', () => {
    const flyAgaric = flyAgaricTemplate as unknown as Graph;
    expect(groupsOf(flyAgaric)).toEqual(['Stipe', 'Cap', 'Gills', 'Veil', 'Cuticle', 'Volva']);
    const group = (name: string) => flyAgaric.groups.find((g) => g.name === name)!;
    const typesIn = (name: string) => group(name).nodes.map((n) => n.type);
    const instance = (name: string) =>
      flyAgaric.nodes.find((n) => n.props?.groupId === group(name).id)!;
    const type = (id: number) => flyAgaric.nodes.find((n) => n.id === id)!.type;
    const into = (name: string) =>
      flyAgaric.edges.filter((e) => e[2] === instance(name).id).map((e) => [type(e[0]), e[3]]);
    const from = (name: string) =>
      flyAgaric.edges.filter((e) => e[0] === instance(name).id).map((e) => type(e[2]));

    // the warts are the patches of the torn veil, the red cuticle shows in the cracks
    expect(typesIn('Cuticle')).toContain('max/myzel/cracks');
    expect(typesIn('Cuticle')).toContain('max/myzel/cap_pattern');
    expect(into('Cuticle')).toContainEqual(['max/myzel/hyphae_to_mesh', 'mesh']);
    // the pattern grows with the radius of the cap, so the warts stay on their spot of it
    expect(into('Cuticle')).toContainEqual(['__internal/group/instance', 'radius']);
    expect(from('Cuticle')).toEqual(['max/myzel/join_meshes']);
    // a membrane on the button, the ring a skirt, no fringe on the margin
    expect(typesIn('Veil').filter((t) => t === 'max/myzel/skirt')).toHaveLength(1);
    expect(typesIn('Veil')).toContain('max/myzel/membrane');
    // the rims of the volva hang from the top of the bulb
    expect(typesIn('Volva')).toContain('max/myzel/skirt');
    expect(into('Volva').map(([, input]) => input).sort()).toEqual(['bulb', 'stipe', 't']);
    expect(from('Volva')).toEqual(['max/myzel/join_meshes']);
  });

  it('lists the Fly agaric with the mushrooms', () => {
    const names = projectTemplates.filter((t) => t.category === 'Mushrooms').map((t) => t.name);
    expect(names).toContain('Fly agaric');
  });
  it('the Porcini has pores instead of gills and a net on its stipe', () => {
    const porcini = porciniTemplate as unknown as Graph;
    expect(groupsOf(porcini)).toEqual(['Stipe', 'Cap', 'Pores', 'Reticulation']);
    const group = (name: string) => porcini.groups.find((g) => g.name === name)!;
    const typesIn = (name: string) => group(name).nodes.map((n) => n.type);
    const instance = (name: string) =>
      porcini.nodes.find((n) => n.props?.groupId === group(name).id)!;
    const type = (id: number) => porcini.nodes.find((n) => n.id === id)!.type;
    const into = (name: string) =>
      porcini.edges.filter((e) => e[2] === instance(name).id).map((e) => [type(e[0]), e[3]]);
    const from = (name: string) =>
      porcini.edges.filter((e) => e[0] === instance(name).id).map((e) => type(e[2]));

    // a sponge of tubes from a net of cracks, colored by time
    expect(typesIn('Pores')).toEqual(expect.arrayContaining([
      'max/myzel/cracks',
      'max/myzel/pores',
      'max/myzel/mix_color',
      'max/myzel/set_mesh_color'
    ]));
    expect(typesIn('Pores')).not.toContain('max/myzel/lamellae');
    expect(into('Pores').map(([, input]) => input).sort()).toEqual(['rays', 'reach', 't']);
    expect(from('Pores')).toEqual(['max/myzel/join_meshes']);
    // the net is cap_pattern around the stipe alone, between hyphae_to_mesh and join_meshes
    expect(typesIn('Reticulation')).toEqual(expect.arrayContaining([
      'max/myzel/cracks',
      'max/myzel/cap_pattern'
    ]));
    expect(into('Reticulation')).toEqual(expect.arrayContaining([
      ['max/myzel/hyphae_to_mesh', 'mesh'],
      ['__internal/group/instance', 'stipe']
    ]));
    const stipeOut = porcini.edges.find((e) =>
      e[2] === instance('Reticulation').id && e[3] === 'stipe'
    )!;
    expect(stipeOut[0]).toBe(instance('Stipe').id);
    expect(stipeOut[1]).toBe(0);
    expect(from('Reticulation')).toEqual(['max/myzel/join_meshes']);
  });

  it('lists the Porcini with the mushrooms', () => {
    const names = projectTemplates.filter((t) => t.category === 'Mushrooms').map((t) => t.name);
    expect(names).toContain('Porcini');
  });
});

describe('bundled graphs', () => {
  const graphs = {
    ...import.meta.glob<Graph>('./graph-templates/*.json', { eager: true, import: 'default' }),
    ...import.meta.glob<Graph>('../../benchmark/templates/*.json', {
      eager: true,
      import: 'default'
    })
  };
  const definitions = Object.values(
    import.meta.glob<NodeDefinition>('../../../nodes/*/*/*/src/definition.json', {
      eager: true,
      import: 'default'
    })
  );
  const inputsOf = new Map(definitions.map((d) => [d.id as string, Object.keys(d.inputs ?? {})]));

  it.each(Object.entries(graphs))('%s is saved in the current version', (_, graph) => {
    expect(graph.version).toBe(CURRENT_GRAPH_VERSION);
  });

  it.each(Object.entries(graphs))('%s only uses inputs its nodes have', (_, graph) => {
    const unknown: string[] = [];
    for (const scope of [graph, ...graph.groups]) {
      const types = new Map(scope.nodes.map((n) => [n.id, n.type as string]));
      const check = (id: number, input: string) => {
        const inputs = inputsOf.get(types.get(id)!);
        if (inputs && !inputs.includes(input)) unknown.push(`${types.get(id)} ${input}`);
      };
      for (const node of scope.nodes) {
        for (const input of [...Object.keys(node.props ?? {}), ...(node.exposed ?? [])]) {
          check(node.id, input);
        }
      }
      for (const edge of scope.edges) check(edge[2], edge[3]);
    }
    expect([...new Set(unknown)]).toEqual([]);
  });

  const outputsOf = new Map(definitions.map((d) => [d.id as string, d.outputs?.length ?? 0]));

  it.each(Object.entries(graphs))('%s only uses outputs its nodes have', (_, graph) => {
    const unknown: string[] = [];
    for (const scope of [graph, ...graph.groups]) {
      const types = new Map(scope.nodes.map((n) => [n.id, n.type as string]));
      for (const [from, output] of scope.edges) {
        const count = outputsOf.get(types.get(from)!);
        if (count !== undefined && output >= count) unknown.push(`${types.get(from)} ${output}`);
      }
    }
    expect([...new Set(unknown)]).toEqual([]);
  });
});
