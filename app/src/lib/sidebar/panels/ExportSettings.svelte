<script lang="ts">
  import { Button, toast } from '@nodarium/ui';
  import FileSaver from 'file-saver';
  import type { Group } from 'three';
  import type { GLTFExporter } from 'three/addons/exporters/GLTFExporter.js';
  import type { OBJExporter } from 'three/addons/exporters/OBJExporter.js';
  import { createExportScene } from './exportScene';

  // Download
  const download = (
    data: ArrayBuffer | string,
    name: string,
    mimetype: string,
    extension: string
  ) => {
    const blob = new Blob([data], { type: mimetype + ';charset=utf-8' });
    FileSaver.saveAs(blob, name + '.' + extension);
  };

  const { scene }: { scene: Group } = $props();

  let gltfExporter: GLTFExporter;
  async function exportGltf() {
    const exporter = gltfExporter
      || (await import('three/addons/exporters/GLTFExporter.js').then((m) => {
        gltfExporter = new m.GLTFExporter();
        return gltfExporter;
      }));

    exporter.parse(
      createExportScene(scene),
      (glb) => {
        download(glb as ArrayBuffer, 'plant', 'model/gltf-binary', 'glb');
        toast('Exported as GLB', 'success');
      },
      (err) => {
        const msg = err instanceof Error ? err.message : String(err);
        toast(`GLB export failed: ${msg}`, 'error');
      },
      { binary: true }
    );
  }

  let objExporter: OBJExporter;

  async function exportObj() {
    const exporter = objExporter
      || (await import('three/addons/exporters/OBJExporter.js').then((m) => {
        objExporter = new m.OBJExporter();
        return objExporter;
      }));
    try {
      const result = exporter.parse(createExportScene(scene));
      download(result, 'plant', 'text/plain', 'obj');
      toast('Exported as OBJ', 'success');
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      toast(`OBJ export failed: ${msg}`, 'error');
    }
  }
</script>

<div class="p-4 flex gap-2">
  <Button onclick={exportObj}>export obj</Button>
  <Button onclick={exportGltf}>export glb</Button>
</div>
