import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { page } from 'vitest/browser';
import InputNumber from './InputNumber.svelte';

describe('InputNumber', () => {
  it('should render input element', async () => {
    render(InputNumber, { value: 0.5 });

    const input = page.getByRole('spinbutton');
    await expect.element(input).toBeInTheDocument();
  });

  it('should render with step buttons when step is provided', async () => {
    render(InputNumber, { value: 0.5, step: 0.1 });

    const decrementBtn = page.getByRole('button', { name: 'step down' });
    const incrementBtn = page.getByRole('button', { name: 'step up' });
    await expect.element(decrementBtn).toBeInTheDocument();
    await expect.element(incrementBtn).toBeInTheDocument();
  });

  it('should not render step buttons when step is undefined', async () => {
    const screen = render(InputNumber, { value: 0.5 });

    const buttons = screen.locator.getByRole('button');
    const count = buttons.all().length;
    expect(count).toBe(0);
  });

  it('should accept numeric value', async () => {
    render(InputNumber, { value: 42 });

    const input = page.getByRole('spinbutton');
    await expect.element(input).toHaveValue(42);
  });

  it('should accept min and max bounds', async () => {
    render(InputNumber, { value: 5, min: 0, max: 10 });

    const input = page.getByRole('spinbutton');
    await expect.element(input).toHaveAttribute('min', '0');
    await expect.element(input).toHaveAttribute('max', '10');
  });

  it('should not clamp value on init when value exceeds min/max', async () => {
    render(InputNumber, { value: 100, min: 0, max: 10 });

    const input = page.getByRole('spinbutton');
    await expect.element(input).toHaveValue(100);
  });

  it('should not clamp value on init when value is below min', async () => {
    render(InputNumber, { value: -50, min: 0, max: 10 });

    const input = page.getByRole('spinbutton');
    await expect.element(input).toHaveValue(-50);
  });
});
