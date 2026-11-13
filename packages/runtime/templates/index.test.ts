import { readdirSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { loadTemplate, templateCategories, templates } from './index';

describe('templates', () => {
  it('are grouped into Plants and Mushrooms', () => {
    expect(templateCategories).toEqual(['Plants', 'Mushrooms']);
    for (const category of templateCategories) {
      expect(templates.some((t) => t.category === category)).toBe(true);
    }
  });

  it('list every json file of the folder once', () => {
    const files = readdirSync(import.meta.dirname)
      .filter((f) => f.endsWith('.json'))
      .map((f) => f.slice(0, -5));
    expect(templates.map((t) => t.id).sort()).toEqual(files.sort());
  });

  it('have unique names', () => {
    const names = templates.map((t) => t.name);
    expect(new Set(names).size).toBe(names.length);
  });

  it('load the graph of their id, with their unit and category', async () => {
    for (const template of templates) {
      const graph = await template.load();
      expect(graph.meta?.unit ?? 'm', template.id).toBe(template.unit);
      const myzel = graph.nodes.some((n) => n.type === 'max/myzel/output');
      expect(template.category, template.id).toBe(myzel ? 'Mushrooms' : 'Plants');
    }
  });

  it('reject an unknown id', async () => {
    await expect(loadTemplate('nope')).rejects.toThrow('Unknown template');
  });
});
