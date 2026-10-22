/** Everything that sets how the gizmos look, apart from the colors, which come from the theme */
export const GIZMO_STYLE = {
  /** drawn on top of the result, like the debug view */
  renderOrder: 999,
  /** the size of the handles of the transform controls, 1 is the default of three.js */
  handleSize: 1,
  /** the fill of the sphere of a force, it only shows where the force reaches */
  forceOpacity: 0.08,
  /** the fill of the dots that mark a position */
  markerOpacity: 0.5,
  /** the wireframe is this many times as opaque as the fill */
  wireframeFactor: 2,
  /** the radius of the dot that marks a position, in world units */
  markerSize: 0.1,
  /** the radius of the marker of a vec3 node, which has no radius of its own */
  positionSize: 0.12,
  /** the thickness of the line between handles, in world units */
  lineThickness: 0.012,
  /** the segments of the sphere, fill and wireframe */
  sphereSegments: { fill: [32, 16], wireframe: [16, 8] }
} as const;
