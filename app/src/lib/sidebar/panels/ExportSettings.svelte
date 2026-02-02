<script lang="ts">
  import FileSaver from 'file-saver';
  import type { Group } from 'three';
  import type { GLTFExporter } from 'three/addons/exporters/GLTFExporter.js';
  import type { OBJExporter } from 'three/addons/exporters/OBJExporter.js';

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

  const { scene } = $props<{ scene: Group }>();

  let gltfExporter: GLTFExporter;
  async function exportGltf() {
    const exporter = gltfExporter
      || (await import('three/addons/exporters/GLTFExporter.js').then((m) => {
        gltfExporter = new m.GLTFExporter();
        return gltfExporter;
      }));

    exporter.parse(
      scene,
      (gltf) => {
        // download .gltf file
        download(gltf as ArrayBuffer, 'plant', 'text/plain', 'gltf');
      },
      (err) => {
        console.log(err);
      }
    );
  }

  let objExporter: OBJExporter;

  async function exportObj() {
    const exporter = objExporter
      || (await import('three/addons/exporters/OBJExporter.js').then((m) => {
        objExporter = new m.OBJExporter();
        return objExporter;
      }));
    const result = exporter.parse(scene);
    // download .obj file
    download(result, 'plant', 'text/plain', 'obj');
  }
</script>

<div class="p-4">
  <button onclick={exportObj}>export obj</button>
  <button onclick={exportGltf}>export gltf</button>
</div>
