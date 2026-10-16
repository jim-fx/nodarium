import * as templates from '$lib/graph-templates';
import { localState } from '$lib/helpers/localState.svelte';
import { ThumbnailService } from '$lib/thumbnails/thumbnail-service.svelte';
import type { Graph } from '@nodarium/types';
import FileSaver from 'file-saver';
import * as db from './project-database.svelte';
import { createProjectFile, parseProjectFile, previewToBlob } from './project-file';

export class ProjectManager {
  public graph = $state<Graph>();
  public readonly thumbnails = new ThumbnailService();
  private projects = $state<Graph[]>([]);
  private activeProjectId = localState<number | undefined>(
    'node.activeProjectId',
    undefined
  );
  public readonly loading = $derived(
    this.projects.length && this.graph?.id !== this.activeProjectId.value
  );

  constructor() {
    this.init();
  }

  async saveGraph(g: Graph) {
    await db.saveGraph(g);
  }

  private async init() {
    await db.getDB();
    this.projects = await db.getGraphs();
    this.thumbnails.init(this.projects);

    if (this.activeProjectId.value !== undefined) {
      const loadedGraph = await db.getGraph(this.activeProjectId.value);
      if (loadedGraph) {
        this.graph = loadedGraph;
      }
    }

    if (!this.graph) {
      if (this.projects?.length && this.projects[0]?.id !== undefined) {
        this.graph = this.projects[0];
        this.activeProjectId.value = this.graph.id;
      }
    }

    if (!this.graph) {
      this.handleCreateProject();
    }
  }

  public handleCreateProject(
    template: Graph = templates.defaultPlant as unknown as Graph,
    title: string = 'New Project'
  ) {
    // templates are shared module objects, never hand them out directly
    const g = structuredClone(template);

    let id = g?.id || 0;
    while (this.projects.find((p) => p.id === id)) {
      id++;
    }

    g.id = id;
    if (!g.meta) g.meta = {};
    g.meta.title = title;

    db.saveGraph(g);
    this.projects = [...this.projects, g];
    this.handleSelectProject(id);
    return id;
  }

  public async handleDeleteProject(projectId: number) {
    await db.deleteGraph(projectId);
    this.thumbnails.remove(projectId);
    if (this.projects.length === 1) {
      this.graph = undefined;
      this.projects = [];
    } else {
      this.projects = this.projects.filter((p) => p.id !== projectId);
      const id = this.projects[0].id;
      if (id !== undefined) {
        this.handleSelectProject(id);
      }
    }
  }

  public async handleSelectProject(id: number) {
    if (this.activeProjectId.value !== id) {
      this.thumbnails.flush();
      const project = await db.getGraph(id);
      this.graph = project;
      this.activeProjectId.value = id;
    }
  }

  public async exportProject(id: number) {
    const graph = await db.getGraph(id);
    if (!graph) return;
    await this.thumbnails.flush();
    const file = await createProjectFile(graph, await this.thumbnails.getBlob(id));
    const title = graph.meta?.title || 'project';
    FileSaver.saveAs(
      new Blob([JSON.stringify(file)], { type: 'application/json' }),
      `${title}.nodarium.json`
    );
  }

  public async importProject(file: File) {
    const { graph, preview } = parseProjectFile(JSON.parse(await file.text()));
    const title = graph.meta?.title || file.name.replace(/(\.nodarium)?\.json$/, '');
    const id = this.handleCreateProject(graph, title);
    if (preview) {
      await this.thumbnails.setPreview(id, await previewToBlob(preview));
    }
  }
}
