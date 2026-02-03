<script lang="ts">
  import { type Snippet } from 'svelte';
  import { type Panel, panelState } from './PanelState.svelte';

  const {
    id,
    icon = '',
    title = '',
    classes = '',
    hidden,
    children
  } = $props<{
    id: string;
    icon?: string;
    title?: string;
    classes?: string;
    hidden?: boolean;
    children?: Snippet;
  }>();

  let panel = $state<Panel>(null!);

  $effect(() => {
    panelState.unregisterPanel(id);
    panel = panelState.registerPanel(id, icon, classes, hidden);
  });

  $effect(() => {
    panel.hidden = hidden;
  });
</script>

{#if panelState.activePanel.value === id}
  <div class="wrapper" class:hidden>
    {#if title}
      <header>
        <h3>{title}</h3>
      </header>
    {/if}
    {@render children?.()}
  </div>
{/if}

<style>
  header {
    border-bottom: solid thin var(--color-outline);
    height: 70px;
    display: flex;
    align-items: center;
    padding-left: 1em;
  }
</style>
