import * as templates from '$lib/graph-templates';
import { localState } from '$lib/helpers/localState.svelte';
import type { Graph } from '@nodarium/types';
import * as db from './project-database.svelte';

export class ProjectManager {
  public graph = $state<Graph>();
  private projects = $state<Graph[]>([]);
  private activeProjectId = localState<number | undefined>(
    'node.activeProjectId',
    undefined
  );
  public readonly loading = $derived(this.graph?.id !== this.activeProjectId.value);

  constructor() {
    this.init();
  }

  async saveGraph(g: Graph) {
    db.saveGraph(g);
  }

  private async init() {
    await db.getDB();
    this.projects = await db.getGraphs();

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
    g: Graph = templates.defaultPlant as unknown as Graph,
    title: string = 'New Project'
  ) {
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
  }

  public async handleDeleteProject(projectId: number) {
    await db.deleteGraph(projectId);
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
      const project = await db.getGraph(id);
      this.graph = project;
      this.activeProjectId.value = id;
    }
  }
}
