<script lang="ts">
  import { defaultPlant, lottaFaces, plant, simple } from '$lib/graph-templates';
  import type { Graph } from '$lib/types';
  import { InputSelect } from '@nodarium/ui';
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
</script>

<header class="flex justify-between px-4 h-[70px] border-b-1 border-outline items-center bg-layer-2">
  <h3>Project</h3>
  <button
    class="px-3 py-1 bg-layer-1 rounded"
    onclick={() => (showNewProject = !showNewProject)}
  >
    New
  </button>
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
    <button
      class="cursor-pointer self-end px-3 py-1 bg-selected rounded"
      onclick={() => handleCreate()}
    >
      Create
    </button>
  </div>
{/if}

<div class="text-white min-h-screen">
  {#if projectManager.loading}
    <p>Loading...</p>
  {/if}

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
              class="text-layer-1! bg-red-500 w-7 text-xl rounded-sm cursor-pointer opacity-20 hover:opacity-80"
              onclick={() => {
                projectManager.handleDeleteProject(project.id!);
              }}
            >
              ×
            </button>
          </div>
        </div>
      </li>
    {/each}
  </ul>
</div>
