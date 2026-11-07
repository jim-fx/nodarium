import { describe, expect, it } from 'vitest';
import { projectTemplates, templateCategories } from './project-templates';

describe('project templates', () => {
  it('are grouped into Plants and Mushrooms', () => {
    expect(templateCategories).toEqual(['Plants', 'Mushrooms']);
    for (const category of templateCategories) {
      expect(projectTemplates.some((t) => t.category === category)).toBe(true);
    }
  });

  it('put every mushroom, a graph with the Myzel output, under Mushrooms', () => {
    for (const template of projectTemplates) {
      const myzel = template.graph.nodes.some((n) => n.type === 'max/myzel/output');
      expect(template.category, template.name).toBe(myzel ? 'Mushrooms' : 'Plants');
    }
  });

  it('have unique names', () => {
    const names = projectTemplates.map((t) => t.name);
    expect(new Set(names).size).toBe(names.length);
  });
});
