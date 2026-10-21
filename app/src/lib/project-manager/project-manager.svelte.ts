import * as templates from '#lib/graph-templates/index.ts';
import { localState } from '#lib/helpers/localState.svelte.ts';
import { ThumbnailService } from '#lib/thumbnails/thumbnail-service.svelte.ts';
import type { Graph } from '@nodarium/types';
import FileSaver from 'file-saver';
import * as db from './project-database.svelte';
import { createProjectFile, parseProjectFile, previewToBlob } from './project-file';

export class ProjectManager {
  public graph = $state<Graph>();
  public readonly thumbnails = new ThumbnailService();
  public projects = $state<Graph[]>([]);
  private activeProjectId = localState<number | undefined>(
    'node.activeProjectId',
    undefined
  );
  public readonly loading = $derived(
    this.projects.length && this.graph?.id !== this.activeProjectId.value
  );
  /** most recently modified first */
  public readonly sortedProjects = $derived(
    [...this.projects].sort((a, b) =>
      (b.meta?.lastModified ?? '').localeCompare(a.meta?.lastModified ?? '')
    )
  );
  public readonly activeProject = $derived(
    this.projects.find((p) => p.id === this.activeProjectId.value)
  );
  /**
   * Called when the open project is renamed, the graph manager has to know
   * about the new title, otherwise its next save overwrites it
   */
  public onrenameactive?: (title: string) => void;

  get activeId() {
    return this.activeProjectId.value;
  }

  constructor() {
    this.init();
  }

  async saveGraph(g: Graph) {
    await db.saveGraph(g);
    this.updateProjectEntry(g);
  }

  private updateProjectEntry(g: Graph) {
    this.projects = this.projects.map((p) => (p.id === g.id ? g : p));
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
    // eslint-disable-next-line svelte/prefer-svelte-reactivity
    g.meta = { ...g.meta, title, lastModified: new Date().toISOString() };

    db.saveGraph(g);
    this.projects = [...this.projects, g];
    this.handleSelectProject(id);
    return id;
  }

  public async handleDeleteProject(projectId: number) {
    const sorted = this.sortedProjects;
    const index = sorted.findIndex((p) => p.id === projectId);
    const neighbour = sorted[index + 1] ?? sorted[index - 1];

    await db.deleteGraph(projectId);
    this.thumbnails.remove(projectId);
    this.projects = this.projects.filter((p) => p.id !== projectId);

    if (!neighbour) {
      this.graph = undefined;
      this.activeProjectId.value = undefined;
      this.handleCreateProject();
    } else if (projectId === this.activeProjectId.value) {
      this.handleSelectProject(neighbour.id);
    }
  }

  public async renameProject(id: number, title: string) {
    title = title.trim();
    const graph = await db.getGraph(id);
    if (!title || !graph || graph.meta?.title === title) return;

    if (id === this.activeProjectId.value) {
      this.onrenameactive?.(title);
    }
    graph.meta = { ...graph.meta, title };
    await this.saveGraph(graph);
  }

  public async duplicateProject(id: number) {
    const graph = await db.getGraph(id);
    if (!graph) return;
    const newId = this.handleCreateProject(graph, `${graph.meta?.title || 'Untitled'} copy`);
    await this.thumbnails.copy(id, newId);
    return newId;
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
