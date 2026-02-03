<script lang="ts">
  import { type Snippet } from 'svelte';
  let { title, value, children, class: _class } = $props<{
    title?: string;
    value?: unknown;
    children?: Snippet;
    class?: string;
  }>();
</script>

<section class="border-outline border-1/2 bg-layer-1 rounded border mb-4 p-4 flex flex-col gap-4 {_class}">
  <h3 class="flex gap-2 font-bold">
    {title}
    <p class="font-normal! opacity-50!">{value}</p>
  </h3>
  <div>
    {@render children()}
  </div>
</section>
