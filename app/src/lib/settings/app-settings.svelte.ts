import { localState } from '$lib/helpers/localState.svelte';

const themes = [
  'dark',
  'light',
  'catppuccin',
  'solarized',
  'high-contrast',
  'high-contrast-light',
  'nord',
  'dracula'
] as const;

export const AppSettingTypes = {
  theme: {
    type: 'select',
    options: themes,
    label: 'Theme',
    value: themes[0]
  },
  showGrid: {
    type: 'boolean',
    label: 'Show Grid',
    value: true
  },
  centerCamera: {
    type: 'boolean',
    label: 'Center Camera',
    value: true
  },
  nodeInterface: {
    title: 'Node Interface',
    backgroundType: {
      type: 'select',
      label: 'Background',
      options: ['grid', 'dots', 'none'],
      value: 'grid'
    },
    snapToGrid: {
      type: 'boolean',
      label: 'Snap to Grid',
      value: true
    },
    showHelp: {
      type: 'boolean',
      label: 'Show Help',
      value: false
    }
  },
  debug: {
    title: 'Debug',
    wireframe: {
      type: 'boolean',
      label: 'Wireframe',
      value: false
    },
    useWorker: {
      type: 'boolean',
      label: 'Execute in WebWorker',
      value: true
    },
    showIndices: {
      type: 'boolean',
      label: 'Show Indices',
      value: false
    },
    advancedMode: {
      type: 'boolean',
      label: 'Advanced Mode',
      value: false
    },
    showVertices: {
      type: 'boolean',
      label: 'Show Vertices',
      value: false
    },
    showStemLines: {
      type: 'boolean',
      label: 'Show Stem Lines',
      value: false
    },
    cache: {
      title: 'Cache',
      useRuntimeCache: {
        type: 'boolean',
        label: 'Node Results',
        value: true
      },
      useRegistryCache: {
        type: 'boolean',
        label: 'Node Source',
        value: true
      }
    },
    stressTest: {
      title: 'Stress Test',
      amount: {
        type: 'integer',
        min: 2,
        max: 15,
        value: 4
      },
      loadGrid: {
        type: 'button',
        label: 'Load Grid'
      },
      loadTree: {
        type: 'button',
        label: 'Load Tree'
      },
      lottaFaces: {
        type: 'button',
        label: "Load 'lots of faces'"
      },
      lottaNodes: {
        type: 'button',
        label: "Load 'lots of nodes'"
      },
      lottaNodesAndFaces: {
        type: 'button',
        label: "Load 'lots of nodes and faces'"
      }
    }
  }
} as const;

type SettingsToStore<T> = T extends { type: 'button' } ? () => void
  : T extends { value: infer V } ? V extends readonly string[] ? V[number]
    : V
  : T extends object ? {
      -readonly [K in keyof T as T[K] extends object ? K : never]: SettingsToStore<T[K]>;
    }
  : never;

export function settingsToStore<T>(settings: T): SettingsToStore<T> {
  const result = {} as Record<string, unknown>;
  for (const key in settings) {
    const value = settings[key];
    if (value && typeof value === 'object') {
      if ('value' in value) {
        result[key] = value.value;
      } else {
        result[key] = settingsToStore(value);
      }
    }
  }
  return result as SettingsToStore<T>;
}

export const appSettings = localState(
  'app-settings',
  settingsToStore(AppSettingTypes)
);

$effect.root(() => {
  $effect(() => {
    const theme = appSettings.value.theme;
    const classes = document.documentElement.classList;
    const newClassName = `theme-${theme}`;
    if (classes) {
      for (const className of classes) {
        if (className.startsWith('theme-') && className !== newClassName) {
          classes.remove(className);
        }
      }
    }
    document.documentElement.classList.add(newClassName);
  });
});
