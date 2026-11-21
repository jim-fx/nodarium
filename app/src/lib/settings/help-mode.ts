import { track } from '#lib/analytics.ts';
import { toast } from '@nodarium/ui';
import { appSettings } from './app-settings.svelte.ts';

/** Turns the help on the nodes on or off, from `?` and from the Help menu */
export function toggleHelpMode() {
  // the inferred settings type has the literal default value, `false`
  const nodeInterface = appSettings.value.nodeInterface as { showHelp: boolean };
  const on = !nodeInterface.showHelp;
  nodeInterface.showHelp = on;
  toast(on ? 'Help mode on' : 'Help mode off', 'info', 1500);
  track(on ? 'help-on' : 'help-off');
}
