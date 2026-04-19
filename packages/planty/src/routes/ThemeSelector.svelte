<script lang="ts">
  import { InputSelect } from '@nodarium/ui';
  const themes = [
    'dark',
    'light',
    'solarized',
    'catppuccin',
    'high-contrast',
    'high-contrast-light',
    'nord',
    'dracula',
    'custom'
  ];

  let { theme = $bindable() } = $props();

  let themeIndex = $state(0);
  $effect(() => {
    theme = themes[themeIndex];
    const classList = document.documentElement.classList;
    for (const c of classList) {
      if (c.startsWith('theme-')) document.documentElement.classList.remove(c);
    }
    document.documentElement.classList.add(`theme-${themes[themeIndex]}`);
  });
</script>

<InputSelect bind:value={themeIndex} options={themes}></InputSelect>
