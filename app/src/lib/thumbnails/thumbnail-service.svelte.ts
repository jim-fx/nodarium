import * as db from '$lib/project-manager/project-database.svelte';
import type { Graph } from '@nodarium/types';
import * as Comlink from 'comlink';
import { SvelteMap } from 'svelte/reactivity';
import { hashGraph } from './graph-hash';
import { createThumbnailWorker } from './thumbnail-worker';

const THUMBNAIL_SIZE = 512;
const PREVIEW_SIZE = 256;
// How long the graph has to stay unchanged before a new thumbnail is rendered
const IDLE_DELAY = 3000;
// Give the active project a head start before rendering missing thumbnails
const BACKFILL_DELAY = 5000;

export class ThumbnailService {
  /** object urls of the stored thumbnails, keyed by project id */
  readonly urls = new SvelteMap<number, string>();

  private worker = createThumbnailWorker();
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  private hashes = new Map<number, string>();
  private pending?: { graph: Graph; result: Int32Array };
  private timeout?: ReturnType<typeof setTimeout>;
  private queue: Promise<void> = Promise.resolve();

  async init(graphs: Graph[]) {
    for (const thumbnail of await db.getThumbnails()) {
      this.hashes.set(thumbnail.id, thumbnail.graphHash);
      this.setUrl(thumbnail.id, thumbnail.blob);
    }
    setTimeout(() => this.backfill(graphs), BACKFILL_DELAY);
  }

  /**
   * Remember the latest execution result of a graph,
   * once the graph stays unchanged for a while a thumbnail is rendered from it
   */
  schedule(graph: Graph, result: Int32Array) {
    this.pending = { graph, result };
    clearTimeout(this.timeout);
    this.timeout = setTimeout(() => this.flush(), IDLE_DELAY);
  }

  /** Render the scheduled thumbnail right away */
  flush() {
    clearTimeout(this.timeout);
    const pending = this.pending;
    this.pending = undefined;
    if (pending) {
      const { graph, result } = pending;
      // the viewer still holds views into the original buffer, so we transfer a copy
      const copy = result.slice();
      this.enqueue(
        graph,
        () => this.worker.renderResult(Comlink.transfer(copy, [copy.buffer]), THUMBNAIL_SIZE)
      );
    }
    return this.queue;
  }

  async getBlob(id: number) {
    return (await db.getThumbnail(id))?.blob;
  }

  /**
   * Store a preview (for example from an imported project file),
   * it gets replaced by a rendered thumbnail on the next backfill/execution
   */
  async setPreview(id: number, blob: Blob) {
    await db.saveThumbnail({ id, blob, graphHash: '' });
    this.hashes.set(id, '');
    this.setUrl(id, blob);
  }

  async copy(from: number, to: number) {
    const thumbnail = await db.getThumbnail(from);
    if (!thumbnail) return;
    await db.saveThumbnail({ ...thumbnail, id: to });
    this.hashes.set(to, thumbnail.graphHash);
    this.setUrl(to, thumbnail.blob);
  }

  /** Render a graph that is not a project (e.g. a template), the result is not stored */
  renderPreview(graph: Graph, size = PREVIEW_SIZE) {
    // $state proxies can not be sent to the worker
    const result = this.queue.then(() => this.worker.renderGraph($state.snapshot(graph), size));
    this.queue = result.then(() => undefined, () => undefined);
    return result.then((blob) => (blob ? URL.createObjectURL(blob) : undefined));
  }

  remove(id: number) {
    const url = this.urls.get(id);
    if (url) URL.revokeObjectURL(url);
    this.urls.delete(id);
    this.hashes.delete(id);
  }

  private backfill(graphs: Graph[]) {
    for (const graph of graphs) {
      this.enqueue(graph, () => this.worker.renderGraph($state.snapshot(graph), THUMBNAIL_SIZE));
    }
  }

  private enqueue(graph: Graph, render: () => Promise<Blob | undefined>) {
    const id = graph.id;
    this.queue = this.queue.then(async () => {
      const graphHash = hashGraph(graph);
      if (this.hashes.get(id) === graphHash) return;
      try {
        const blob = await render();
        // the project might have been deleted while rendering
        if (!blob || !(await db.getGraph(id))) return;
        await db.saveThumbnail({ id, blob, graphHash });
        this.hashes.set(id, graphHash);
        this.setUrl(id, blob);
      } catch (error) {
        console.warn(`[thumbnails] failed to render thumbnail for project ${id}`, error);
      }
    });
  }

  private setUrl(id: number, blob: Blob) {
    const oldUrl = this.urls.get(id);
    if (oldUrl) URL.revokeObjectURL(oldUrl);
    this.urls.set(id, URL.createObjectURL(blob));
  }
}
