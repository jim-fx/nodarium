import type { PlantyConfig } from '@nodarium/planty';

export const tutorialConfig: PlantyConfig = {
  id: 'nodarium-tutorial',
  avatar: {
    name: 'Planty',
    defaultPosition: 'bottom-right'
  },
  start: 'intro',
  nodes: {
    // ── Entry ──────────────────────────────────────────────────────────────
    intro: {
      position: 'center',
      text:
        "# Hi, I'm Planty! 🌱\nI'll show you around Nodarium — a tool for building 3D plants by connecting nodes together.\nHow much detail do you want?",
      choices: [
        { label: '🌱 Show me the basics', next: 'tour_canvas' },
        { label: '🤓 I want the technical details', next: 'tour_canvas_nerd' },
        { label: 'Skip the tour for now', next: null }
      ]
    },

    // ── Simple path ────────────────────────────────────────────────────────

    tour_canvas: {
      position: 'bottom-left',
      hook: 'setup-default',
      highlight: { selector: '.graph-wrapper', padding: 12 },
      text:
        'This is the **graph canvas**. Nodes connect together to build a plant — the 3D model updates automatically whenever you make a change.\nEach node does one specific job: grow stems, add noise, produce output.',
      waitFor: 'click',
      next: 'tour_viewer'
    },

    tour_viewer: {
      position: 'top-left',
      highlight: { selector: '.cell:first-child', padding: 8 },
      text:
        'This is the **3D viewer** — the live preview of your plant.\nLeft-click to rotate · right-click to pan · scroll to zoom.',
      waitFor: 'click',
      next: 'try_params'
    },

    try_params: {
      position: 'bottom-right',
      highlight: { selector: '.graph-wrapper', padding: 12 },
      text:
        'Click a node to select it. Its settings appear **directly on the node** — try changing a value and watch the plant update.\nThe sidebar shows extra hidden settings for the selected node.',
      waitFor: 'click',
      next: 'start_building'
    },

    start_building: {
      position: 'center',
      hook: 'load-tutorial-template',
      text:
        "Now let's build your own plant from scratch!\nI've loaded a blank project — it only has an **Output** node. Your goal: connect nodes to make a plant.",
      waitFor: 'click',
      next: 'add_stem_node'
    },

    add_stem_node: {
      position: 'bottom-right',
      highlight: { selector: '.graph-wrapper', padding: 12 },
      text:
        "Open the **Add Menu** with **Shift+A** or **right-click** on the canvas.\nAdd a **Stem** node, then connect its output socket to the Output node's input.",
      waitFor: 'click',
      next: 'add_noise_node'
    },

    add_noise_node: {
      position: 'bottom-right',
      highlight: { selector: '.graph-wrapper', padding: 12 },
      text:
        'Add a **Noise** node the same way.\nConnect: Stem → Noise input, then Noise output → Output.\nThis makes the stems grow in organic, curved shapes.',
      waitFor: 'click',
      next: 'add_random_node'
    },

    add_random_node: {
      position: 'bottom-right',
      highlight: { selector: '.graph-wrapper', padding: 12 },
      text:
        "Let's add some randomness! Add a **Random** node and connect its output to the **thickness** or **length** input of the Stem node.\nThe default min/max range is small — **Ctrl+drag** any number field to exceed its normal limits.",
      waitFor: 'click',
      next: 'prompt_regenerate'
    },

    prompt_regenerate: {
      position: 'bottom-right',
      highlight: { selector: '.graph-wrapper', padding: 12 },
      text:
        'Now press **R** to regenerate. Each press gives the Random node a new value — your plant changes every run!',
      waitFor: 'click',
      next: 'tour_sidebar'
    },

    tour_sidebar: {
      position: 'right',
      highlight: { selector: '.tabs', padding: 4 },
      text:
        'The **sidebar** holds all your tools:\n⚙️ Settings · ⌨️ Shortcuts · 📦 Export · 📁 Projects · 📊 Graph Settings\nEnable **Advanced Mode** in Settings to unlock performance and benchmark panels.',
      waitFor: 'click',
      next: 'save_project'
    },

    save_project: {
      position: 'right',
      highlight: { selector: '.tabs', padding: 4 },
      text:
        'Your work is saved in the **Projects panel** — rename it, create new projects, or switch between them anytime.',
      waitFor: 'click',
      next: 'congrats'
    },

    congrats: {
      position: 'center',
      text:
        "# You're all set! 🎉\nYou know how to build plants, tweak parameters, and save your work.\nWant to explore more?",
      choices: [
        { label: '🔗 How do node connections work?', next: 'connections_intro' },
        { label: '💡 Ideas for improving this plant', next: 'improvements_hint' },
        { label: '⌨️ Keyboard shortcuts', next: 'shortcuts_tour' },
        { label: "I'm ready to build!", next: null }
      ]
    },

    // ── Technical / nerd path ──────────────────────────────────────────────

    tour_canvas_nerd: {
      position: 'bottom-left',
      hook: 'setup-default',
      highlight: { selector: '.graph-wrapper', padding: 12 },
      text:
        "The **graph canvas** renders a directed acyclic graph. Each node is an individual **WASM module** executed in isolation — inputs in, output out. The 3D model updates automatically on every change.\nI've loaded a starter graph so you can see it in action.",
      choices: [
        {
          label: '🔍 Explore Node Sourcecode',
          onclick: () => {
            window.open(
              'https://git.max-richter.dev/max/nodarium/src/branch/main/nodes/max/plantarium',
              '__blank'
            );
          }
        }
      ],
      waitFor: 'click',
      next: 'tour_viewer_nerd'
    },

    tour_viewer_nerd: {
      position: 'top-left',
      highlight: { selector: '.cell:first-child', padding: 8 },
      text:
        'The **3D viewer** uses `@threlte/core` (Svelte + Three.js). Mesh data streams from WASM execution results. OrbitControls: left-drag rotate, right-drag pan, scroll zoom.',
      waitFor: 'click',
      next: 'tour_runtime_nerd'
    },

    tour_runtime_nerd: {
      position: 'right',
      text:
        'By default, nodes execute in a **WebWorker** for better performance. You can switch to main-thread execution by disabling **Debug → Execute in WebWorker** in Settings.\nEnable **Advanced Mode** to unlock the Performance and Benchmark panels.',
      waitFor: 'click',
      next: 'start_building'
    },

    // ── Deep dives (shared between paths) ─────────────────────────────────

    connections_intro: {
      position: 'bottom-right',
      text:
        'Node sockets are **type-checked**. The coloured dots tell you what kind of data flows through:\n🔵 `number` · 🟢 `vec3` · 🟣 `shape` · ⚪ `*` (wildcard)',
      waitFor: 'click',
      next: 'connections_rules'
    },

    connections_rules: {
      position: 'right',
      text:
        'Drag from an output socket to an input socket to connect them.\n• Types must match (or use `*`)\n• No circular loops\n• Optional inputs can stay empty\nInvalid connections snap back automatically.',
      choices: [
        { label: '🔧 Node parameters', next: 'params_intro' },
        { label: '🐛 Debug node', next: 'debug_intro' },
        { label: 'Start building!', next: null }
      ]
    },

    params_intro: {
      position: 'bottom-right',
      highlight: { selector: '.graph-wrapper', padding: 12 },
      text:
        'Click any node to select it. Basic settings are shown **on the node itself**.\nThe sidebar under *Graph Settings → Active Node* shows the full list:\n**Number** — drag or type · **Vec3** — X/Y/Z · **Select** — dropdown · **Color** — picker',
      waitFor: 'click',
      next: 'params_tip'
    },

    params_tip: {
      position: 'right',
      text:
        'Pro tips:\n• Parameters can be connected from other nodes — drag an edge to the input socket\n• The **Random Seed** in Graph Settings gives you the same result every run\n• **f** key smart-connects two selected nodes · **Ctrl+Delete** removes a node and restores its edges',
      choices: [
        { label: '🔗 How connections work', next: 'connections_intro' },
        { label: '💡 Plant improvement ideas', next: 'improvements_hint' },
        { label: 'Start building!', next: null }
      ]
    },

    debug_intro: {
      position: 'bottom-right',
      text:
        'Add a **Debug node** from the Add Menu (Shift+A or right-click). It accepts `*` wildcard inputs — connect any socket to inspect the data flowing through.\nEnable **Advanced Mode** in Settings to also see Performance and Graph Source panels.',
      waitFor: 'click',
      next: 'debug_done'
    },

    debug_done: {
      position: 'center',
      text: 'The Debug node is your best friend when building complex graphs.\nAnything else?',
      choices: [
        { label: '🔗 Connection types', next: 'connections_intro' },
        { label: '🔧 Node parameters', next: 'params_intro' },
        { label: 'Start building!', next: null }
      ]
    },

    shortcuts_tour: {
      position: 'bottom-right',
      text:
        '**Essential shortcuts:**\n`R` — Regenerate\n`Shift+A` / right-click — Add node\n`f` — Smart-connect selected nodes\n`.` — Center camera\n`Ctrl+Z` / `Ctrl+Y` — Undo / Redo\n`Delete` — Remove selected · `Ctrl+Delete` — Remove and restore edges',
      waitFor: 'click',
      next: 'shortcuts_done'
    },

    shortcuts_done: {
      position: 'right',
      text:
        'All shortcuts are also listed in the sidebar under the ⌨️ icon.\nReady to build something?',
      choices: [
        { label: '🔗 Node connections', next: 'connections_intro' },
        { label: '🔧 Parameters', next: 'params_intro' },
        { label: "Let's build! 🌿", next: null }
      ]
    },

    export_tour: {
      position: 'right',
      text:
        'Export your 3D model from the **📦 Export** panel:\n**GLB** — standard for 3D apps (Blender, Three.js)\n**OBJ** — legacy format · **STL** — 3D printing · **PNG** — screenshot',
      waitFor: 'click',
      next: 'congrats'
    },

    improvements_hint: {
      position: 'center',
      text:
        '# Ideas to grow your plant 🌿\n• Add a **Vec3** node → connect to *origin* on the Stem to spread stems across 3D space\n• Use a **Random** node on a parameter so each run produces a unique shape\n• Chain **multiple Stem nodes** with different settings for complex branching\n• Add a **Gravity** or **Branch** node for even more organic results',
      choices: [
        { label: '⌨️ Keyboard shortcuts', next: 'shortcuts_tour' },
        { label: "Let's build! 🌿", next: null }
      ]
    }
  }
};
