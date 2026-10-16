<script lang="ts">
  import { formatRelativeTime } from '$lib/helpers/formatRelativeTime';
  import type { ProjectManager } from '$lib/project-manager/project-manager.svelte';
  import { appSettings, AppSettingTypes } from '$lib/settings/app-settings.svelte';
  import { panelState } from '$lib/sidebar/PanelState.svelte';
  import { ConfirmDialog, type Menu, MenuBar, type MenuEntry, toast } from '@nodarium/ui';
  import { tick } from 'svelte';

  const RECENT_PROJECTS = 5;

  type Props = {
    projectManager: ProjectManager;
    saving: boolean;
    onopenprojects: () => void;
    onstarttutorial: () => void;
  };

  const { projectManager: pm, saving, onopenprojects, onstarttutorial }: Props = $props();

  const title = $derived(pm.activeProject?.meta?.title || 'Untitled');

  let renaming = $state(false);
  let renameValue = $state('');
  let renameInput = $state<HTMLInputElement>();

  export async function startRename() {
    if (pm.activeId === undefined) return;
    renameValue = title;
    renaming = true;
    await tick();
    renameInput?.focus();
    renameInput?.select();
  }

  function finishRename(commit: boolean) {
    if (!renaming) return;
    renaming = false;
    if (commit && pm.activeId !== undefined) {
      pm.renameProject(pm.activeId, renameValue);
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
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      toast(`Import failed: ${msg}`, 'error');
    }
  }

  let confirmDelete = $state(false);

  function withActive(fn: (id: number) => void) {
    return () => {
      if (pm.activeId !== undefined) fn(pm.activeId);
    };
  }

  const recentItems = $derived.by((): MenuEntry[] => {
    const recent = pm.sortedProjects
      .filter((p) => p.id !== pm.activeId)
      .slice(0, RECENT_PROJECTS)
      .map((p) => ({
        label: p.meta?.title || 'Untitled',
        image: pm.thumbnails.urls.get(p.id) ?? '',
        detail: formatRelativeTime(p.meta?.lastModified),
        onselect: () => pm.handleSelectProject(p.id)
      }));
    const browse = { label: 'Browse all projects…', shortcut: 'Ctrl+O', onselect: onopenprojects };
    return recent.length ? [...recent, 'separator', browse] : [browse];
  });

  // the inferred settings type uses the literal default values, so we need a looser view
  type ViewSettings = {
    showGrid: boolean;
    centerCamera: boolean;
    theme: string;
    nodeInterface: { snapToGrid: boolean };
  };
  const settings = $derived(appSettings.value as unknown as ViewSettings);

  const menus = $derived<Menu[]>([
    {
      label: 'File',
      items: [
        { label: 'New project…', shortcut: 'Alt+N', onselect: onopenprojects },
        { label: 'Open…', shortcut: 'Ctrl+O', onselect: onopenprojects },
        { label: 'Open recent', items: recentItems },
        'separator',
        { label: 'Rename', shortcut: 'F2', onselect: startRename },
        { label: 'Duplicate', onselect: withActive((id) => pm.duplicateProject(id)) },
        'separator',
        { label: 'Import .json…', onselect: () => importInput?.click() },
        { label: 'Export .json', onselect: withActive((id) => pm.exportProject(id)) },
        { label: 'Export mesh…', onselect: () => panelState.setActivePanel('exports') },
        'separator',
        { label: 'Delete project…', onselect: () => (confirmDelete = true) }
      ]
    },
    {
      label: 'View',
      items: [
        {
          label: 'Sidebar',
          checked: !!panelState.activePanel.value,
          onselect: () => panelState.toggleOpen()
        },
        {
          label: 'Show grid',
          checked: settings.showGrid,
          onselect: () => (settings.showGrid = !settings.showGrid)
        },
        {
          label: 'Center camera',
          checked: settings.centerCamera,
          onselect: () => (settings.centerCamera = !settings.centerCamera)
        },
        {
          label: 'Snap to grid',
          checked: settings.nodeInterface.snapToGrid,
          onselect: () => (settings.nodeInterface.snapToGrid = !settings.nodeInterface.snapToGrid)
        },
        'separator',
        {
          label: 'Theme',
          items: AppSettingTypes.theme.options.map((theme) => ({
            label: theme,
            checked: settings.theme === theme,
            onselect: () => (settings.theme = theme)
          }))
        }
      ]
    },
    {
      label: 'Help',
      items: [
        { label: 'Tutorial', onselect: onstarttutorial },
        { label: 'Keyboard shortcuts', onselect: () => panelState.setActivePanel('shortcuts') },
        { label: 'Changelog', onselect: () => panelState.setActivePanel('changelog') },
        'separator',
        {
          label: 'Source code',
          onselect: () => window.open('https://github.com/jim-fx/nodarium', '_blank')
        }
      ]
    }
  ]);
</script>

<header class="title-bar relative flex h-full items-stretch border-b border-outline bg-layer-1">
  <div class="flex items-center border-r border-outline px-2.5">
    <img src="/favicon.svg" alt="Nodarium" class="block h-5 w-5" />
  </div>
  <MenuBar {menus} />

  <div class="absolute left-1/2 top-0 flex h-full -translate-x-1/2 items-center gap-3.5">
    {#if renaming}
      <input
        bind:this={renameInput}
        bind:value={renameValue}
        aria-label="Project name"
        class="h-6 w-64 rounded-[3px] border border-outline bg-layer-0 px-2 text-center text-sm outline-none focus:border-selected"
        onkeydown={(e) => {
          if (e.key === 'Enter') finishRename(true);
          if (e.key === 'Escape') finishRename(false);
        }}
        onblur={() => finishRename(true)}
      />
    {:else}
      <button
        type="button"
        title="Rename (F2)"
        class="max-w-80 cursor-pointer truncate border-none bg-transparent px-1 text-sm font-(--font-family)"
        onclick={startRename}
      >
        {title}
      </button>
    {/if}
    <span class="text-xs opacity-50" aria-live="polite">{saving ? 'saving…' : 'saved'}</span>
  </div>

  <button
    type="button"
    aria-label="Open Planty"
    title="Open Planty"
    class="ml-auto flex w-[30px] cursor-pointer items-center justify-center border-y-0 border-r-0 border-l border-outline bg-transparent p-0 hover:bg-layer-2"
    onclick={onstarttutorial}
  >
    <span class="block h-[18px] w-[18px] i-[tabler--seedling]"></span>
  </button>

  <input
    bind:this={importInput}
    type="file"
    accept=".json,application/json"
    class="hidden"
    onchange={handleImport}
  />
</header>

<ConfirmDialog
  bind:open={confirmDelete}
  title="Delete project?"
  message={`"${title}" and all its data will be permanently removed.`}
  confirmLabel="Delete"
  cancelLabel="Cancel"
  onconfirm={withActive((id) => pm.handleDeleteProject(id))}
/>
