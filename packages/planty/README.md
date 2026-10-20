# @nodarium/planty

Planty is a small in-app tour guide for Svelte 5. A dialog graph describes the steps; each step
can move the avatar, highlight an element, run an app action and wait for the user to do
something before it continues.

## Usage

```svelte
<script lang="ts">
  import { Planty, type PlantyConfig } from '@nodarium/planty';

  const config: PlantyConfig = {
    id: 'my-tour',
    start: 'hello',
    nodes: {
      hello: {
        position: 'center',
        text: '# Hi!\nWant a tour?',
        choices: [
          { label: 'Yes', next: 'canvas' },
          { label: 'No thanks', next: null }
        ]
      },
      canvas: {
        position: 'bottom-left',
        highlight: { selector: '#canvas', padding: 12 },
        text: 'Add a node to continue.'
      }
    }
  };

  let planty: ReturnType<typeof Planty>;
</script>

<Planty
  bind:this={planty}
  {config}
  autoStart
  hooks={{
    'action:canvas': (advance) => editor.on('node-added', advance)
  }}
/>
<button onclick={() => planty.start()}>Tutorial</button>
```

## Config

| Field                 | Description                                                                                         |
| --------------------- | --------------------------------------------------------------------------------------------------- |
| `id`                  | Unique id, also used for the `planty:<id>:seen` localStorage key of `autoStart`                     |
| `start`               | Id of the first node                                                                                |
| `avatar`              | `name` (used as the bubble's accessible label) and `defaultPosition`                                |
| `nodes[id].text`      | Bubble text. Supports `# heading`, `**bold**`, `*italic*`, `` `code` `` and line breaks             |
| `nodes[id].position`  | An anchor (`top-left`, `top-right`, `bottom-left`, `bottom-right`, `center`, `right`) or `{ x, y }` |
| `nodes[id].highlight` | `{ selector }` or `{ hookName }` plus optional `padding`                                            |
| `nodes[id].action`    | Name of an entry in `actions` to run when the node is entered                                       |
| `nodes[id].next`      | Id of the next node, shows a **Next** button                                                        |
| `nodes[id].choices`   | Buttons with `next` (navigate, `null` ends the tour) or `action` (run an action and stay)           |

A node without `next` or `choices` ends the tour after a few seconds. `DialogRunner#validate()`
lists dangling links and unreachable nodes, Planty also logs them as a warning.

## Props

| Prop           | Description                                                                         |
| -------------- | ----------------------------------------------------------------------------------- |
| `config`       | The dialog graph                                                                    |
| `actions`      | `Record<string, () => void \| Cleanup>`, referenced by `action` in nodes or choices |
| `hooks`        | Step hooks, see below                                                               |
| `autoStart`    | Start on mount unless the tour has been started before in this browser              |
| `onStepChange` | Called with the id and node whenever a node is entered                              |
| `onComplete`   | Called when the tour is closed or ends                                              |

`bind:this` exposes `start()`, `stop()`, `next()` and `registerHook(name, fn)`.

### Hooks

| Key             | Signature                      | When                                                 |
| --------------- | ------------------------------ | ---------------------------------------------------- |
| `before:<node>` | `(id, node) => void`           | Before the avatar moves to the node                  |
| `after:<node>`  | `(id, node) => void`           | When the user leaves the node                        |
| `action:<node>` | `(advance) => void \| Cleanup` | After the node is shown, call `advance()` to move on |
| `<hookName>`    | `() => Element \| null`        | Resolves the element for a `{ hookName }` highlight  |

Cleanups returned by actions and `action:` hooks run as soon as the user leaves the node, closes
the tour or restarts it, so listeners never outlive their step.

## Keyboard

**Enter** finishes the typing animation or goes to the next step, **Esc** closes the tour.
Clicking the text also finishes the animation. Animations are skipped when the user prefers
reduced motion.

## Development

```sh
pnpm dev     # demo page in src/routes
pnpm check
pnpm lint
```
