# @nodarium/ui

The Svelte 5 components of [Nodarium](https://nodes.max-richter.dev): the inputs of the nodes (`Input`, `InputNumber`, `InputCurve`, `InputShape`, `InputVec3`, ...), menus, tabs, dialogs, toasts and the keymap, with the theme of the app. `@nodarium/graph-interface`, the node editor, is built on it.

## Install

The package is in the npm registry of git.max-richter.dev. Add the scope to the `.npmrc` of your project:

```
@nodarium:registry=https://git.max-richter.dev/api/packages/max/npm/
```

```sh
npm install @nodarium/ui @nodarium/runtime
```

`svelte` 5, `tailwindcss` 4 and `@nodarium/runtime` (for the types of the inputs) are peer dependencies. The package ships `.svelte` files, use it with Vite and `@sveltejs/vite-plugin-svelte` or SvelteKit.

## Styles

The components are styled with Tailwind classes. `@nodarium/ui/app.css` is a Tailwind entry that builds the classes of the components, the theme (`--color-layer-0` to `--color-layer-3`, `--color-text`, `--color-outline`, `--color-active`, `--color-selected`, `--color-error`, `--color-frame-*`, with `bg-layer-1` and the other utilities of these colours) and the font, Fira Code, which comes with the package. Import it once, in the root layout:

```svelte
<script lang="ts">
  import '@nodarium/ui/app.css';
</script>
```

The themes are classes on `<html>`: none for dark, `theme-light`, `theme-solarized`, `theme-catppuccin`, `theme-high-contrast`. Icons are classes of `@iconify/tailwind4` with the prefix `i`, like `i-[tabler--settings]`.

The utilities of the theme colours, like `bg-layer-1` or `text-selected`, are built by that entry, so they work in your own components too. Other Tailwind classes of your components come from your own Tailwind entry.

## Use

```svelte
<script lang="ts">
  import { InputNumber, Toast, toast } from '@nodarium/ui';
  let value = $state(0.5);
</script>

<InputNumber bind:value min={0} max={1} step={0.01} />
<button onclick={() => toast('saved')}>save</button>
<Toast />
```

`Input` takes a `NodeInput` of `@nodarium/runtime` and shows the input of its type, as the nodes of the editor do. `pnpm -C packages/ui dev` in the repository shows every component.
