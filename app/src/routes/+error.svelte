<script lang="ts">
  import { page } from '$app/state';
</script>

<main class="w-screen h-screen flex flex-col items-center justify-center">
  <div class="outline-1 outline-outline bg-layer-2">
    <h1 class="p-8 text-3xl">@nodarium/error</h1>
    <hr>
    <pre class="p-8">{JSON.stringify(page.error, null, 2)}</pre>
    <hr>
    <div class="flex p-4">
      <button
        class="bg-layer-2 outline-1 outline-outline p-3 px-6 rounded-sm cursor-pointer"
        on:click={() => window.location.reload()}
      >
        reload
      </button>
    </div>
  </div>
</main>
