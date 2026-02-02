import { localState } from '$lib/helpers/localState.svelte';

export type Panel = {
  icon: string;
  classes: string;
  hidden?: boolean;
};

class PanelState {
  panels = $state<Record<string, Panel>>({});
  activePanel = localState<string | boolean>('node.activePanel', '');

  get keys() {
    return Object.keys(this.panels);
  }

  public unregisterPanel(id: string) {
    delete this.panels[id];
  }

  public registerPanel(id: string, icon: string, classes: string, hidden: boolean): Panel {
    const state = $state({
      icon: icon,
      classes: classes,
      hidden: hidden
    });
    this.panels[id] = state;
    return state;
  }

  public toggleOpen() {
    if (this.activePanel.value) {
      this.activePanel.value = false;
    } else {
      this.activePanel.value = this.keys[0];
    }
  }

  public setActivePanel(panelId: string) {
    this.activePanel.value = panelId;
  }
}

export const panelState = new PanelState();
