<script lang="ts">
  import { type Snippet } from 'svelte';
  let { title, value, header, children, class: _class } = $props<{
    title?: string;
    value?: unknown;
    header?: Snippet;
    children?: Snippet;
    class?: string;
  }>();
</script>

<section class="border-outline border-1/2 bg-layer-1 rounded border mb-4 p-4 flex flex-col gap-4 {_class}">
  <h3 class="flex gap-2 font-bold">
    {title}
    <div class="flex gap-4 w-full font-normal opacity-50 max-w-[75%]">
      {#if header}
        {@render header()}
      {:else}
        {value}
      {/if}
    </div>
  </h3>
  <div>
    {@render children()}
  </div>
</section>
