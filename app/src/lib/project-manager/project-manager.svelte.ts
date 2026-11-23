import * as templates from '#lib/graph-templates/index.ts';
import { localState } from '#lib/helpers/localState.svelte.ts';
import { hashGraph } from '#lib/thumbnails/graph-hash.ts';
import { ThumbnailService } from '#lib/thumbnails/thumbnail-service.svelte.ts';
import { collectSettings } from '@nodarium/graph-interface';
import { type GraphIssue, parseProjectFile } from '@nodarium/runtime';
import { loadTemplate } from '@nodarium/runtime/templates';
import type { Graph, NodeRegistry, ProjectUnit } from '@nodarium/types';
import FileSaver from 'file-saver';
import { findImportIssues, reportImportIssues } from './import-issues';
import * as db from './project-database.svelte';
import { createProjectFile, previewToBlob } from './project-file';
import { projectLink } from './project-link';

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
  public onunitactive?: (unit: ProjectUnit) => void;

  get activeId() {
    return this.activeProjectId.value;
  }

  /**
   * @param registry the nodes an imported graph is checked against
   * @param open opens a project before the last one is, like the graph of a link, and gives its id
   */
  constructor(
    private registry?: NodeRegistry,
    open?: (pm: ProjectManager) => Promise<number | undefined>
  ) {
    this.init(open);
  }

  async saveGraph(g: Graph) {
    await db.saveGraph(g);
    this.updateProjectEntry(g);
  }

  private updateProjectEntry(g: Graph) {
    this.projects = this.projects.map((p) => (p.id === g.id ? g : p));
  }

  private async init(open?: (pm: ProjectManager) => Promise<number | undefined>) {
    await db.getDB();
    this.projects = await db.getGraphs();
    this.thumbnails.init(this.projects);

    const opened = await open?.(this);
    if (opened !== undefined) {
      await this.handleSelectProject(opened);
      if (this.graph) return;
    }

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
      this.handleCreateProject(await loadTemplate('default-plant'));
    }
  }

  public handleCreateProject(
    template: Graph = templates.stem as unknown as Graph,
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

  public async setProjectUnit(id: number, unit: ProjectUnit) {
    const graph = await db.getGraph(id);
    if (!graph || graph.meta?.unit === unit) return;

    if (id === this.activeProjectId.value) {
      this.onunitactive?.(unit);
    }
    graph.meta = { ...graph.meta, unit };
    await this.saveGraph(graph);
  }

  public async duplicateProject(id: number) {
    const graph = await db.getGraph(id);
    if (!graph) return;
    const newId = this.handleCreateProject(graph, `${graph.meta?.title || 'Untitled'} copy`);
    await this.thumbnails.copy(id, newId);
    return newId;
  }

  #opening = $state(0);
  /** A project is read from the database to be opened */
  get opening() {
    return this.#opening > 0;
  }

  public async handleSelectProject(id: number) {
    if (this.activeProjectId.value !== id || this.graph?.id !== id) {
      this.thumbnails.flush();
      this.#opening++;
      try {
        const project = await db.getGraph(id);
        this.graph = project;
        this.activeProjectId.value = id;
      } finally {
        this.#opening--;
      }
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

  /** The link that opens the project as a new project in this app, see `graphLink()` */
  public async projectLink(id: number) {
    const graph = await db.getGraph(id);
    if (!graph) return;
    return projectLink(graph, location.origin);
  }

  /**
   * Opens the project of the file as a new project. A graph with issues is opened all the same,
   * they are shown when a registry is given.
   */
  public async importProject(file: File) {
    const input = JSON.parse(await file.text());
    await this.importGraph(input, { fallbackTitle: file.name.replace(/(\.nodarium)?\.json$/, '') });
  }

  /**
   * Opens a project file or a bare graph as a new project, as `importProject` does with a file.
   * The title is `title`, else the one of the graph, else `fallbackTitle`. `settings` change
   * those of the graph. With `reuse`, a project that has the same graph is opened instead, a
   * link opened twice makes one project.
   */
  public async importGraph(
    input: unknown,
    { title, fallbackTitle = 'New Project', settings, reuse = false }: {
      title?: string;
      fallbackTitle?: string;
      settings?: Record<string, unknown>;
      reuse?: boolean;
    } = {}
  ): Promise<{ id: number; issues: GraphIssue[] }> {
    const { graph, preview } = parseProjectFile(input);
    if (settings) graph.settings = { ...graph.settings, ...settings };
    let issues: GraphIssue[] = [];
    if (this.registry) {
      const written = (isProjectFile(input) ? input.graph : input) as Graph;
      issues = await findImportIssues(written, this.registry).catch((error) => {
        console.error('could not check the imported graph', error);
        return [];
      });
      reportImportIssues(written, issues);
    }
    if (reuse) {
      // the editor fills in the settings of the graph, a project it saved has all of them
      const definitions = [...graph.nodes, ...graph.groups.flatMap((g) => g.nodes)]
        .flatMap((node) => this.registry?.getNode(node.type) ?? []);
      graph.settings = collectSettings(definitions, graph.settings).values;
      const hash = hashGraph(graph, { ignoreKeyOrder: true });
      const same = this.sortedProjects.find((p) => hashGraph(p, { ignoreKeyOrder: true }) === hash);
      if (same) {
        await this.handleSelectProject(same.id);
        return { id: same.id, issues };
      }
    }
    const id = this.handleCreateProject(graph, title || graph.meta?.title || fallbackTitle);
    if (preview) {
      await this.thumbnails.setPreview(id, await previewToBlob(preview));
    }
    return { id, issues };
  }
}

function isProjectFile(input: unknown): input is { graph: unknown } {
  return typeof input === 'object' && input !== null && 'format' in input;
}
