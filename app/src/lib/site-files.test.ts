import { templates } from '@nodarium/runtime/templates';
import { describe, expect, it } from 'vitest';
import { siteDoc, templateIndex } from './site-files';

describe('the files of the site', () => {
  it('list every template of the project browser with its graph and how to open it', async () => {
    const index = await templateIndex();
    expect(index.map((t) => t.id)).toEqual(templates.map((t) => t.id));
    expect(index.find((t) => t.id === 'fern')).toMatchObject({
      name: 'Fern',
      category: 'Plants',
      unit: 'cm',
      species: 'Nephrolepis exaltata (Boston fern)',
      notes: expect.stringContaining('22 fronds'),
      graph: '/templates/fern.json',
      open: '/?template=fern'
    });
    expect(index.every((t) => t.nodes > 2)).toBe(true);
  });

  it('point the links of a doc to the other docs into the repository', () => {
    expect(siteDoc('[MYZEL.md](./MYZEL.md), [site](https://x.dev/a.md)')).toBe(
      '[MYZEL.md](https://git.max-richter.dev/max/nodarium/src/branch/main/docs/MYZEL.md), '
        + '[site](https://x.dev/a.md)'
    );
  });
});
