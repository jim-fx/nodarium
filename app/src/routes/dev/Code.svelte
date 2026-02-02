<script lang="ts">
  import wabtInit from 'wabt';

  const { wasm } = $props<{ wasm: ArrayBuffer }>();

  async function toWat(arrayBuffer: ArrayBuffer) {
    const wabt = await wabtInit();

    const module = wabt.readWasm(new Uint8Array(arrayBuffer), {
      readDebugNames: true
    });

    module.generateNames();
    module.applyNames();

    return module.toText({ foldExprs: false, inlineExport: false });
  }
</script>

{#await toWat(wasm)}
  <p>Converting to WAT</p>
{:then c}
  <pre>
    <code class="text-gray-50">{c}</code>
  </pre>
{/await}
