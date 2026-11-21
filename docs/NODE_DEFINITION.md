# Node Definition

Which arguments a node expects is defined in a single `.json` file that is embedded into our node. It consists of three main fields:

## id

`user_name/namespace/node_id`

Every part of the id and every input name is snake_case: lowercase letters, digits and `_`, starting with a letter. `nodarium_definition_file!` fails the build otherwise. Use `label` on an input when the editor should show a different text.

## outputs

```json
"outputs": [
  "geometry"
]
```

A node can have several outputs. Give them a label to tell outputs of the same type apart on the node:

```json
"outputs": [
  "geometry",
  { "type": "float", "label": "height" }
]
```

The first output sits on the header of the node, every other output gets its own row below it.

## inputs

```json
"inputs": {
  "height": {
    "type": "float",
    "value": 2
  },
  "radius": {
    "type": "float",
    "value": 0.5
  }
}
```

A `float` or `vec3` that is a length gets `"unit": "m"` and one that is an angle `"unit": "rad"`. The node always gets meters and radians, the editor shows lengths in the unit of the project and angles in degrees or radians, as the user chose under View → Angles. Write the angles in the description of a node or an input as `1.5708 rad` and say `in radians`, the editor shows them as `90°` and `in degrees` then.

A `text` input holds a string, like the rules of `lsystem`. It shows a text field on the node, `"multiline": true` a field of several lines. The node gets one word per Unicode code point and reads it with `read_text` of `nodarium_utils`. No node gives text, so a text input is `"internal": true`, without a socket. A graph stores it as a string in `props`.

Saved graphs refer to inputs by their name. When you rename or remove an input, add a migration in `packages/runtime/src/graph-migrations` so graphs that use the old name keep their values and links.

## meta (optional)

The meta object is optional and can contain two other fields:

`title`

`description`
