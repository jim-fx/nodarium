import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { page } from 'vitest/browser';
import InputCheckbox from './InputCheckbox.svelte';

describe('InputCheckbox', () => {
  it('should render checkbox label', async () => {
    render(InputCheckbox, { value: false });

    const checkbox = page.getByRole('checkbox');
    await expect.element(checkbox).toBeInTheDocument();
  });

  it('should be unchecked when value is false', async () => {
    render(InputCheckbox, { value: false });

    const input = page.getByRole('checkbox');
    await expect.element(input).not.toBeChecked();
  });

  it('should be checked when value is true', async () => {
    render(InputCheckbox, { value: true });

    const input = page.getByRole('checkbox');
    await expect.element(input).toBeChecked();
  });
});
