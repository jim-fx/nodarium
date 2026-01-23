import { localState } from '$lib/helpers/localState.svelte';
import { settingsToStore } from '$lib/settings/app-settings.svelte';

export const DevSettingsType = {
  debugNode: {
    type: 'boolean',
    label: 'Debug Nodes',
    value: true
  }
} as const;

export let devSettings = localState(
  'dev-settings',
  settingsToStore(DevSettingsType)
);
