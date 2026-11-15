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

  it('shows 1 as checked and leaves the value as it is', async () => {
    const props = $state({ value: 1 as unknown as boolean });
    render(InputCheckbox, props);

    await expect.element(page.getByRole('checkbox')).toBeChecked();
    expect(props.value).toBe(1);
  });

  it('writes a boolean when the user changes it', async () => {
    const props = $state({ value: 1 as unknown as boolean });
    render(InputCheckbox, props);

    await page.getByRole('checkbox').click({ force: true });
    expect(props.value).toBe(false);
  });
});
