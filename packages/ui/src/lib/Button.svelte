<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    variant?: 'default' | 'primary' | 'destructive' | 'ghost';
    size?: 'sm' | 'md';
    disabled?: boolean;
    class?: string;
    onclick?: (e: MouseEvent) => void;
    children?: Snippet;
    type?: 'button' | 'submit' | 'reset';
  }

  let {
    variant = 'default',
    size = 'md',
    disabled = false,
    class: _class = '',
    onclick,
    children,
    type = 'button'
  }: Props = $props();

  const variantClasses = {
    default: 'bg-layer-2 border border-outline text-text hover:opacity-85',
    primary: 'bg-selected text-white border border-transparent hover:opacity-88',
    destructive: 'bg-red-600 text-white border border-transparent hover:opacity-88',
    ghost: 'bg-layer-2 border border-transparent text-text opacity-75 hover:opacity-100'
  };

  const sizeClasses = {
    sm: 'px-2 py-0.5 text-xs',
    md: 'px-3 py-1 text-sm'
  };
</script>

<button
  {type}
  {disabled}
  class:py-1={size === 'sm'}
  class:px-1={size === 'sm'}
  class:py-2={size !== 'sm'}
  class="
    inline-flex items-center gap-1.5 rounded cursor-pointer
    font-(--font-family) leading-none whitespace-nowrap
    transition-opacity duration-100
    disabled:opacity-40 disabled:cursor-not-allowed
    {variantClasses[variant]}
    {sizeClasses[size]}
    {_class}
  "
  {onclick}
>
  {@render children?.()}
</button>
