import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { page } from 'vitest/browser';
import Details from './Details.svelte';

describe('Details', () => {
  it('should render summary element', async () => {
    render(Details, { title: 'Click me' });

    const summary = page.getByText('Click me');
    await expect.element(summary).toBeInTheDocument();
  });
});
