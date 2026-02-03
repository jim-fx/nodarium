import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { page } from 'vitest/browser';
import InputSelect from './InputSelect.svelte';

describe('InputSelect', () => {
  it('should render select element', async () => {
    render(InputSelect, { options: ['a', 'b', 'c'], value: 0 });

    const select = page.getByRole('combobox');
    await expect.element(select).toBeInTheDocument();
  });

  it('should render all options', async () => {
    render(InputSelect, { options: ['apple', 'banana', 'cherry'], value: 0 });

    const select = page.getByRole('combobox');
    await expect.element(select).toHaveTextContent('applebananacherry');
  });

  it('should select correct option by index', async () => {
    render(InputSelect, { options: ['first', 'second', 'third'], value: 1 });

    const select = page.getByRole('combobox');
    await expect.element(select).toHaveTextContent('second');
  });
});
