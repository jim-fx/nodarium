<script lang="ts">
  import { formatRelativeTime } from '$lib/helpers/formatRelativeTime';
  import { localState } from '$lib/helpers/localState.svelte';
  import type { Graph } from '@nodarium/types';
  import {
    Button,
    ConfirmDialog,
    InputCheckbox,
    InputSelect,
    type MenuEntry,
    MenuList,
    toast
  } from '@nodarium/ui';
  import { onMount, tick } from 'svelte';
  import type { ProjectManager } from './project-manager.svelte';
  import { type ProjectTemplate, projectTemplates } from './project-templates';

  type Props = {
    projectManager: ProjectManager;
    open?: boolean;
  };

  let { projectManager: pm, open = $bindable(false) }: Props = $props();

  const showOnStartup = localState('node.projectBrowser.showOnStartup', false);

  onMount(() => {
    if (showOnStartup.value) open = true;
  });

  let dialogEl = $state<HTMLDialogElement>();
  let search = $state('');
  let sortBy = $state(0);
  let selectedId = $state<number>();

  $effect(() => {
    if (!dialogEl) return;
    if (open && !dialogEl.open) {
      search = '';
      selectedId = pm.activeId;
      dialogEl.showModal();
      loadTemplatePreviews();
    } else if (!open && dialogEl.open) {
      dialogEl.close();
    }
  });

  const projects = $derived.by(() => {
    const query = search.trim().toLowerCase();
    const filtered = pm.sortedProjects.filter((p) =>
      (p.meta?.title || 'Untitled').toLowerCase().includes(query)
    );
    if (sortBy === 1) {
      filtered.sort((a, b) => (a.meta?.title ?? '').localeCompare(b.meta?.title ?? ''));
    }
    return filtered;
  });

  let templatePreviews = $state<Record<string, string>>({});
  let templatePreviewsRequested = false;

  function loadTemplatePreviews() {
    if (templatePreviewsRequested) return;
    templatePreviewsRequested = true;
    for (const template of projectTemplates) {
      pm.thumbnails.renderPreview(template.graph).then((url) => {
        if (url) templatePreviews[template.name] = url;
      });
    }
  }

  function close() {
    open = false;
    contextMenu = undefined;
  }

  function openProject(id: number | undefined) {
    if (id === undefined) return;
    pm.handleSelectProject(id);
    close();
  }

  function createFromTemplate(template: ProjectTemplate) {
    pm.handleCreateProject(template.graph, template.name);
    close();
  }

  let renamingId = $state<number>();
  let renameValue = $state('');
  let renameInput = $state<HTMLInputElement>();

  async function startRename(project: Graph) {
    renamingId = project.id;
    renameValue = project.meta?.title || '';
    await tick();
    renameInput?.focus();
    renameInput?.select();
  }

  function finishRename(commit: boolean) {
    if (renamingId === undefined) return;
    if (commit) pm.renameProject(renamingId, renameValue);
    renamingId = undefined;
  }

  let pendingDelete = $state<Graph>();
  let confirmDelete = $state(false);

  function requestDelete(project: Graph) {
    pendingDelete = project;
    confirmDelete = true;
  }

  let contextMenu = $state<{ project: Graph; x: number; y: number }>();

  const contextMenuItems = $derived.by((): MenuEntry[] => {
    if (!contextMenu) return [];
    const { project } = contextMenu;
    return [
      { label: 'Open', shortcut: 'Enter', onselect: () => openProject(project.id) },
      { label: 'Rename', shortcut: 'F2', onselect: () => startRename(project) },
      { label: 'Duplicate', onselect: () => pm.duplicateProject(project.id) },
      { label: 'Export .json', onselect: () => pm.exportProject(project.id) },
      'separator',
      { label: 'Delete…', shortcut: 'Del', onselect: () => requestDelete(project) }
    ];
  });

  function handleContextMenu(event: MouseEvent, project: Graph) {
    event.preventDefault();
    selectedId = project.id;
    contextMenu = { project, x: event.clientX, y: event.clientY };
  }

  function handleCardKeydown(event: KeyboardEvent, project: Graph) {
    if (event.key === 'Enter') {
      event.preventDefault();
      openProject(project.id);
    } else if (event.key === 'F2') {
      event.preventDefault();
      startRename(project);
    } else if (event.key === 'Delete') {
      event.preventDefault();
      requestDelete(project);
    }
  }

  let importInput = $state<HTMLInputElement>();

  async function handleImport(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file) return;
    try {
      await pm.importProject(file);
      close();
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      toast(`Import failed: ${msg}`, 'error');
    }
  }
</script>

{#snippet thumbnail(url: string | undefined, classes: string)}
  <span class="block shrink-0 bg-layer-0 {classes}">
    {#if url}
      <img src={url} alt="" class="block h-full w-full object-contain" />
    {/if}
  </span>
{/snippet}

<dialog
  bind:this={dialogEl}
  aria-label="Projects"
  class="
    m-auto h-[min(680px,calc(100vh-32px))] max-h-none w-[min(1080px,calc(100vw-32px))] max-w-none
    overflow-hidden rounded-md border border-outline bg-layer-1 p-0 text-text font-(--font-family)
    backdrop:bg-black/50
  "
  oncancel={(e) => {
    e.preventDefault();
    close();
  }}
  onclick={(e) => {
    if (e.target === dialogEl) close();
  }}
  onpointerdown={(e) => {
    if (contextMenu && !(e.target as HTMLElement).closest('[role="menu"]')) {
      contextMenu = undefined;
    }
  }}
>
  <div class="grid h-full grid-cols-[260px_1fr] grid-rows-[70px_1fr_60px]">
    <header class="col-span-2 flex items-center gap-4 border-b border-outline bg-layer-2 px-5">
      <h3 class="m-0 text-base font-normal">Projects</h3>
      <label
        class="ml-auto flex h-8.5 w-70 items-center gap-2 rounded-[5px] bg-layer-1 px-3 outline outline-outline"
      >
        <span class="block h-4 w-4 shrink-0 opacity-50 i-[tabler--search]"></span>
        <input
          bind:value={search}
          type="text"
          placeholder="search"
          aria-label="Search projects"
          class="w-full border-none bg-transparent text-sm outline-none font-(--font-family)"
        />
      </label>
      <button
        type="button"
        aria-label="Close"
        class="flex h-8.5 w-8.5 cursor-pointer items-center justify-center border-none bg-transparent"
        onclick={close}
      >
        <span class="block h-5 w-5 i-[tabler--x]"></span>
      </button>
    </header>

    <aside class="row-span-2 flex min-h-0 flex-col border-r border-outline">
      <div class="border-b border-outline px-5 pb-2.5 pt-4.5 text-xs opacity-50">
        New from template
      </div>
      <div class="min-h-0 overflow-y-auto">
        {#each projectTemplates as template (template.name)}
          <button
            type="button"
            class="flex h-14 w-full cursor-pointer items-center gap-3 border-x-0 border-t-0 border-b border-outline bg-transparent px-5 text-left text-sm hover:bg-layer-2 font-(--font-family)"
            onclick={() => createFromTemplate(template)}
          >
            {@render thumbnail(templatePreviews[template.name], 'h-[34px] w-[34px] border border-outline')}
            {template.name}
          </button>
        {/each}
      </div>
      <button
        type="button"
        class="mt-auto flex h-15 w-full cursor-pointer items-center gap-3 border-x-0 border-b-0 border-t border-outline bg-transparent px-5 text-left text-sm hover:bg-layer-2 font-(--font-family)"
        onclick={() => importInput?.click()}
      >
        <span class="block h-4 w-4 i-[tabler--upload]"></span>
        Import .json…
      </button>
    </aside>

    <section class="flex min-h-0 flex-col gap-3.5 px-5 pt-4.5">
      <div class="flex items-center justify-end text-xs">
        <InputSelect options={['last modified', 'name']} bind:value={sortBy} />
      </div>
      <ul
        aria-label="Projects"
        class="m-0 grid min-h-0 list-none grid-cols-[repeat(auto-fill,minmax(180px,1fr))] content-start gap-3.5 overflow-y-auto p-0 pb-4.5"
      >
        {#each projects as project (project.id)}
          {@const url = pm.thumbnails.urls.get(project.id)}
          {@const selected = selectedId === project.id}
          <li
            class="
              flex flex-col overflow-hidden rounded-[3px] border bg-layer-1 {selected
              ? 'border-selected'
              : 'border-outline'}
            "
          >
            {#if renamingId === project.id}
              <div class="flex h-8 items-center border-b border-outline bg-layer-2 px-1.5">
                <input
                  bind:this={renameInput}
                  bind:value={renameValue}
                  aria-label="Project name"
                  class="h-6 w-full rounded-[3px] border border-selected bg-layer-0 px-1.5 text-xs outline-none font-(--font-family)"
                  onkeydown={(e) => {
                    if (e.key === 'Enter') finishRename(true);
                    if (e.key === 'Escape') {
                      e.preventDefault();
                      finishRename(false);
                    }
                  }}
                  onblur={() => finishRename(true)}
                />
              </div>
              {@render thumbnail(url, 'h-36 w-full')}
            {:else}
              <button
                type="button"
                class="flex w-full cursor-pointer flex-col border-none bg-transparent p-0 text-left font-(--font-family)"
                onclick={() => (selectedId = project.id)}
                ondblclick={() => openProject(project.id)}
                onkeydown={(e) => handleCardKeydown(e, project)}
                oncontextmenu={(e) => handleContextMenu(e, project)}
              >
                <span
                  class="flex h-8 w-full items-center gap-2 border-b border-outline bg-layer-2 px-2.5 text-xs"
                >
                  <span
                    class="rounded-[2px] bg-outline px-1 text-[11px]"
                    title="{project.nodes.length} nodes"
                  >{project.nodes.length}</span>
                  <span class="truncate">{project.meta?.title || 'Untitled'}</span>
                </span>
                {@render thumbnail(url, 'h-36 w-full p-2 box-border')}
              </button>
            {/if}
            <span
              class="flex h-6.5 items-center border-t border-outline px-2.5 text-[11px] opacity-50"
            >
              {formatRelativeTime(project.meta?.lastModified)}
            </span>
          </li>
        {:else}
          <li class="col-span-full py-8 text-center text-sm opacity-50">
            {search ? `No projects match "${search}"` : 'No projects yet'}
          </li>
        {/each}
      </ul>
    </section>

    <footer class="flex items-center gap-2 border-t border-outline px-5">
      <label class="flex cursor-pointer items-center gap-2.5 text-sm">
        <InputCheckbox bind:value={showOnStartup.value} />
        Show on startup
      </label>
      <span class="ml-auto"></span>
      <Button onclick={close}>Cancel</Button>
      <Button
        variant="primary"
        disabled={selectedId === undefined}
        onclick={() => openProject(selectedId)}
      >
        Open
      </Button>
    </footer>
  </div>

  {#if contextMenu}
    <MenuList
      items={contextMenuItems}
      label="Project actions"
      autofocus
      class="fixed"
      style="left: {contextMenu.x}px; top: {contextMenu.y}px"
      onclose={() => (contextMenu = undefined)}
    />
  {/if}

  <input
    bind:this={importInput}
    type="file"
    accept=".json,application/json"
    class="hidden"
    onchange={handleImport}
  />
</dialog>

<ConfirmDialog
  bind:open={confirmDelete}
  title="Delete project?"
  message={`"${pendingDelete?.meta?.title || 'Untitled'}" and all its data will be permanently removed.`}
  confirmLabel="Delete"
  cancelLabel="Cancel"
  onconfirm={() => pendingDelete && pm.handleDeleteProject(pendingDelete.id)}
/>
