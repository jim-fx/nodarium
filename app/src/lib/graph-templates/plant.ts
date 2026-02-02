export const plant = {
  'settings': { 'resolution.circle': 26, 'resolution.curve': 39 },
  'nodes': [
    { 'id': 9, 'position': [180, 80], 'type': 'max/plantarium/output', 'props': {} },
    {
      'id': 10,
      'position': [55, 80],
      'type': 'max/plantarium/stem',
      'props': { 'amount': 1, 'length': 11, 'thickness': 0.71 }
    },
    {
      'id': 11,
      'position': [80, 80],
      'type': 'max/plantarium/noise',
      'props': {
        'strength': 35,
        'scale': 4.6,
        'fixBottom': 1,
        'directionalStrength': [1, 0.74, 0.083],
        'depth': 1
      }
    },
    {
      'id': 12,
      'position': [105, 80],
      'type': 'max/plantarium/branch',
      'props': {
        'length': 3,
        'thickness': 0.6,
        'amount': 10,
        'rotation': 180,
        'offsetSingle': 0.34,
        'lowestBranch': 0.53,
        'highestBranch': 1,
        'depth': 1
      }
    },
    {
      'id': 13,
      'position': [130, 80],
      'type': 'max/plantarium/noise',
      'props': {
        'strength': 8,
        'scale': 7.7,
        'fixBottom': 1,
        'directionalStrength': [1, 0, 1],
        'depth': 1
      }
    },
    {
      'id': 14,
      'position': [155, 80],
      'type': 'max/plantarium/gravity',
      'props': {
        'strength': 0.11,
        'scale': 39,
        'fixBottom': 0,
        'directionalStrength': [1, 1, 1],
        'depth': 1,
        'curviness': 1
      }
    }
  ],
  'edges': [
    [10, 0, 11, 'plant'],
    [11, 0, 12, 'plant'],
    [12, 0, 13, 'plant'],
    [13, 0, 14, 'plant'],
    [14, 0, 9, 'input']
  ]
};
