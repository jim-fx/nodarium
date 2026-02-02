<script lang="ts">
  import { type Snippet } from 'svelte';
  let { title, value, children } = $props<{
    title?: string;
    value?: unknown;
    children?: Snippet;
  }>();
</script>

<section class="border border-1/2 mb-4 p-4 flex flex-col gap-4">
  <h3 class="flex gap-2 font-bold">
    {title}
    <p class="font-normal! opacity-50!">{value}</p>
  </h3>
  <div>
    {@render children()}
  </div>
</section>
