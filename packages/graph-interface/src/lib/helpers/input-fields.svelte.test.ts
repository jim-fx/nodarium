import type { NodeInput } from '@nodarium/runtime';
import { Input } from '@nodarium/ui';
import { describe, expect, it } from 'vitest';
import { hasInputField } from './nodeHelpers';

const inputs = [
  { type: 'float', value: 1 },
  { type: 'integer', value: 1 },
  { type: 'boolean', value: false },
  { type: 'select', options: ['a', 'b'], value: 0 },
  { type: 'vec3', value: [0, 0, 0] },
  { type: 'color', value: [0, 0, 0] },
  { type: 'shape', value: [0, 0, 1, 1] },
  { type: 'curve', value: [0, 0, 1, 1] },
  { type: 'path' },
  { type: 'geometry' },
  { type: 'fx', value: [] }
] as unknown as NodeInput[];

// the fields only exist in a browser, `vitest --browser=false` has no document for them
describe.skipIf(import.meta.env.SSR)('hasInputField', () => {
  it('names exactly the types Input of @nodarium/ui draws a field for', async () => {
    const { render } = await import('vitest-browser-svelte');
    for (const input of inputs) {
      const { container, unmount } = await render(Input, { input, value: input.value });
      expect(container.childElementCount > 0, input.type).toBe(hasInputField(input.type));
      unmount();
    }
  });
});
