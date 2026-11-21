<script lang="ts">
  interface Props {
    value?: string;
    id?: string;
    /** a field of several lines, Enter starts a new line */
    multiline?: boolean;
  }

  let { value = $bindable(''), id, multiline = false }: Props = $props();

  const field =
    'w-full box-border rounded-sm border border-outline bg-layer-2 px-2 py-1 font-mono text-sm text-text outline-none focus:border-active';
</script>

{#if multiline}
  <textarea
    {id}
    bind:value
    rows="5"
    spellcheck="false"
    autocomplete="off"
    class="{field} block resize-none leading-snug whitespace-pre"
  ></textarea>
{:else}
  <input
    {id}
    type="text"
    bind:value
    spellcheck="false"
    autocomplete="off"
    class={field}
  />
{/if}
