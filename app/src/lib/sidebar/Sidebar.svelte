<script lang="ts">
  import { type Snippet } from 'svelte';
  import { panelState as state } from './PanelState.svelte';

  // eslint-disable-next-line no-useless-assignment
  let { children, open = $bindable(false) } = $props<{ children?: Snippet; open?: boolean }>();

  $effect(() => {
    open = !!state.activePanel.value;
  });
</script>

<div class="wrapper" class:visible={state.activePanel.value}>
  <div class="tabs">
    <button
      class="toggle"
      aria-label={state.activePanel.value ? 'Close sidebar' : 'Open sidebar'}
      title={state.activePanel.value ? 'Close sidebar' : 'Open sidebar'}
      aria-expanded={!!state.activePanel.value}
      onclick={() => state.toggleOpen()}
    >
      <span
        class="
          block w-5 h-5 {state.activePanel.value
          ? 'i-[tabler--layout-sidebar-right-collapse]'
          : 'i-[tabler--layout-sidebar-right-expand]'}
        "
      ></span>
    </button>
    {#each state.keys as panelId (panelId)}
      {#if !state.panels[panelId].hidden}
        <button
          aria-label={panelId}
          class="tab {state.panels[panelId].classes}"
          class:active={panelId === state.activePanel.value}
          onclick={() => (state.activePanel.value = panelId)}
        >
          <span class={`block w-6 h-6 iconify ${state.panels[panelId].icon}`}></span>
        </button>
      {/if}
    {/each}
  </div>
  <div class="content">
    {@render children?.()}
  </div>
</div>

<style>
  .wrapper {
    top: 0px;
    position: absolute;
    display: grid;
    z-index: 2;
    grid-template-columns: 30px 1fr;
    height: 100%;
    right: 0px;
    transform: translateX(calc(100% - 30px));
    transition: transform 0.2s, background 0.2s ease;
    width: 30%;
    min-width: 350px;
  }

  .content {
    background: var(--color-layer-1);
    z-index: 10;
    position: relative;
    max-height: 100%;
    overflow-y: auto;
  }

  .tabs {
    display: flex;
    flex-direction: column;
    border-right: solid thin var(--color-outline);
  }

  .tabs > button {
    height: 35px;
    padding: 5px;
    border-radius: 0px;
    background: none;
    border: none;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    border-bottom: solid thin var(--color-outline);
    border-left: solid thin var(--color-outline);
    background: var(--color-layer-1);
  }

  .tabs > button > span {
    opacity: 0.5;
  }

  .tabs > button:hover > span {
    opacity: 1;
  }

  .tabs > button.active {
    background: var(--color-layer-2);
  }

  .tabs > button.active span {
    opacity: 1;
  }

  .visible .tabs {
    margin-left: -1px;
  }

  .visible {
    transform: translateX(0);
  }
</style>
