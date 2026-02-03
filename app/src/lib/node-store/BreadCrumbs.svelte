<script lang="ts">
  import { InputSelect } from '@nodarium/ui';

  let activeStore = $state(0);
  let { activeId }: { activeId: string } = $props();
  const [activeUser, activeCollection, activeNode] = $derived(
    activeId.split(`/`)
  );
</script>

<div class="breadcrumbs">
  {#if activeUser}
    <InputSelect id="root" options={['root']} bind:value={activeStore}></InputSelect>
    {#if activeCollection}
      <button
        onclick={() => {
          activeId = activeUser;
        }}
      >
        {activeUser}
      </button>
      {#if activeNode}
        <button
          onclick={() => {
            activeId = `${activeUser}/${activeCollection}`;
          }}
        >
          {activeCollection}
        </button>
        <span>{activeNode}</span>
      {:else}
        <span>{activeCollection}</span>
      {/if}
    {:else}
      <span>{activeUser}</span>
    {/if}
  {:else}
    <InputSelect id="root" options={['root']} bind:value={activeStore}></InputSelect>
  {/if}
</div>

<style>
  .breadcrumbs {
    display: flex;
    align-items: center;
    padding: 0.4em;
    gap: 0.8em;
    height: 35px;
    box-sizing: border-box;
    border-bottom: solid thin var(--color-outline);
  }
  .breadcrumbs > button {
    position: relative;
    background: none;
    font-family: var(--font-family);
    border: none;
    font-size: 1em;
    padding: 0px;
    cursor: pointer;
  }

  .breadcrumbs > button::after,
  .breadcrumbs :global(select)::after {
    content: "/";
    position: absolute;
    right: -11px;
    opacity: 0.5;
    white-space: pre;
    text-decoration: none !important;
  }

  .breadcrumbs > button:hover {
    text-decoration: underline;
  }

  .breadcrumbs > span {
    font-size: 1em;
    opacity: 0.5;
  }

  .breadcrumbs :global(select) {
    padding: 3px 5px;
    outline: none;
  }
</style>
