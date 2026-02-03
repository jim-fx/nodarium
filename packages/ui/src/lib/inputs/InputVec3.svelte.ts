import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { page } from 'vitest/browser';
import InputVec3 from './InputVec3.svelte';

describe('InputVec3', () => {
  it('should render with correct initial values', async () => {
    const value = $state([1.5, 2.5, 3.5]);
    render(InputVec3, { value });

    const inputs = page.getByRole('spinbutton');
    await expect.element(inputs.first()).toBeInTheDocument();

    expect(inputs.nth(0)).toHaveValue(1.5);
    expect(inputs.nth(1)).toHaveValue(2.5);
    expect(inputs.nth(2)).toHaveValue(3.5);
  });

  it('should have step attribute', async () => {
    const value = $state([0, 0, 0]);
    render(InputVec3, { value });

    const input = page.getByRole('spinbutton').first();
    await expect.element(input).toHaveAttribute('step', '0.01');
  });
});
