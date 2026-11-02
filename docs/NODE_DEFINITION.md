# Node Definition

Which arguments a node expects is defined in a single `.json` file that is embedded into our node. It consists of three main fields:

## id

`user-name/namespace/node-id`

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

## meta (optional)

The meta object is optional and can contain two other fields:

`title`

`description`
