<script lang="ts">
  import { defaultPlant, lottaFaces, plant, simple } from '$lib/graph-templates';
  import type { Graph } from '$lib/types';
  import { Button, ConfirmDialog, InputSelect, Spinner } from '@nodarium/ui';
  import type { ProjectManager } from './project-manager.svelte';

  const { projectManager } = $props<{ projectManager: ProjectManager }>();

  let showNewProject = $state(false);
  let newProjectName = $state('');

  const templates = [
    {
      name: 'Default Plant',
      value: 'defaultPlant',
      graph: defaultPlant as unknown as Graph
    },
    { name: 'Plant', value: 'plant', graph: plant as unknown as Graph },
    { name: 'Simple', value: 'simple', graph: simple as unknown as Graph },
    {
      name: 'Lotta Faces',
      value: 'lottaFaces',
      graph: lottaFaces as unknown as Graph
    }
  ];
  let selectedTemplateIndex = $state(0);

  function handleCreate() {
    const template = templates[selectedTemplateIndex] || templates[0];
    projectManager.handleCreateProject(template.graph, newProjectName);
    newProjectName = '';
    showNewProject = false;
  }

  let pendingDeleteId = $state<number | null>(null);
  let confirmOpen = $state(false);

  function requestDelete(id: number, e: MouseEvent) {
    e.stopPropagation();
    pendingDeleteId = id;
    confirmOpen = true;
  }

  function confirmDelete() {
    if (pendingDeleteId !== null) {
      projectManager.handleDeleteProject(pendingDeleteId);
      pendingDeleteId = null;
    }
  }
</script>

<header class="flex justify-between px-4 h-[70px] border-b-1 border-outline items-center bg-layer-2">
  <h3>Project</h3>
  <Button onclick={() => (showNewProject = !showNewProject)}>New</Button>
</header>

{#if showNewProject}
  <div class="flex flex-col px-4 py-3.5 mt-[1px] border-b-1 border-outline gap-3">
    <input
      type="text"
      bind:value={newProjectName}
      placeholder="Project name"
      class="w-full px-2 py-2 bg-layer-2 rounded"
      onkeydown={(e) => e.key === 'Enter' && handleCreate()}
    />
    <InputSelect options={templates.map(t => t.name)} bind:value={selectedTemplateIndex} />
    <Button variant="primary" class="self-end" onclick={() => handleCreate()}>Create</Button>
  </div>
{/if}

<div class="text-white min-h-screen">
  <ul>
    {#each projectManager.projects as project (project.id)}
      <li>
        <div
          class="
            h-[70px] border-b-1 border-b-outline
            flex
            w-full text-left px-3 py-2 cursor-pointer {projectManager
            .activeProjectId.value === project.id
            ? 'border-l-2 border-l-selected pl-2.5!'
            : ''}
          "
          onclick={() => projectManager.handleSelectProject(project.id!)}
          role="button"
          tabindex="0"
          onkeydown={(e) =>
          e.key === 'Enter'
          && projectManager.handleSelectProject(project.id!)}
        >
          <div class="flex justify-between items-center grow">
            <span>{project.meta?.title || 'Untitled'}</span>
            <button
              class="opacity-20 hover:opacity-70 transition-opacity cursor-pointer p-1 rounded text-red-400"
              onclick={(e) => requestDelete(project.id!, e)}
              aria-label="Delete project"
            >
              <span class="i-[tabler--trash] w-4 h-4 block"></span>
            </button>
          </div>
        </div>
      </li>
    {:else}
      {#if projectManager.loading}
        <div class="flex items-center gap-2 p-4">
          <Spinner size={12} />
          <p>Loading</p>
        </div>
      {:else}
        <li class="px-4 py-8 text-center opacity-40 text-sm">
          No projects yet.<br />Press <b>New</b> to create one.
        </li>
      {/if}
    {/each}
  </ul>
</div>

<ConfirmDialog
  bind:open={confirmOpen}
  title="Delete project?"
  message="This cannot be undone. The project and all its data will be permanently removed."
  confirmLabel="Delete"
  cancelLabel="Cancel"
  onconfirm={confirmDelete}
/>
