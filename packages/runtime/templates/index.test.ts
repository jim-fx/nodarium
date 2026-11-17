import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { loadTemplate, templateCategories, templates, unlistedTemplates } from './index';

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
    const ids = [...templates, ...unlistedTemplates].map((t) => t.id);
    expect(ids.sort()).toEqual(files.sort());
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

  it('load an unlisted template by its id', async () => {
    for (const template of unlistedTemplates) {
      expect(templates.some((t) => t.id === template.id)).toBe(false);
      expect(await loadTemplate(template.id)).toBe(await template.load());
    }
  });

  it('say what they are, as the doc they name describes them', () => {
    const docs = {
      'plant-templates': readFileSync(
        join(import.meta.dirname, '../../../docs/PLANT_TEMPLATES.md'),
        'utf8'
      ),
      myzel: readFileSync(join(import.meta.dirname, '../../../docs/MYZEL.md'), 'utf8')
    };
    for (const template of [...templates, ...unlistedTemplates]) {
      if (template.id === 'default-plant') {
        expect(template.notes).toBeUndefined();
        continue;
      }
      expect(template.notes, template.id).toMatch(/^[A-Z0-9].+\.$/);
      expect(template.doc, template.id).toBe(
        template.category === 'Plants' ? 'plant-templates' : 'myzel'
      );
      if (template.category === 'Plants') {
        const latin = template.species!.split(' (')[0];
        expect(docs['plant-templates'], template.id).toMatch(
          new RegExp(`^## ${template.name.replace(/[()']/g, '.')}: _${latin}_`, 'm')
        );
      } else {
        expect(docs.myzel, template.id).toContain(`| \`${template.id}\``);
      }
    }
  });

  it('reject an unknown id', async () => {
    await expect(loadTemplate('nope')).rejects.toThrow('Unknown template');
  });
});
