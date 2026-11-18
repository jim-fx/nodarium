import { Color, type Material, ShaderMaterial } from 'three';

/** How a part is shown, the number travels to the shader as it is */
export const HIGHLIGHT = {
  none: 0,
  hover: 1,
  /** made by the selected node */
  selected: 2,
  /** the picked path among the parts of the selected node */
  focus: 3,
  dimmed: 4,
  hidden: 5
} as const;

export type HighlightMode = (typeof HIGHLIGHT)[keyof typeof HIGHLIGHT];

/**
 * Which parts of the result are highlighted. Paths are meshes of their own, instances are told
 * apart by the path they sit on, the tissues of a Myzel mesh by the graph node that made them.
 */
export type PartHighlight = {
  /** by path id, for the geometry of the path */
  paths: Map<number, HighlightMode>;
  /** by path id, for the instances that sit on the path */
  instances: Map<number, HighlightMode>;
  /** for everything else */
  rest: HighlightMode;
  /** names what is selected, the tint of a selection that fades starts over when it changes */
  selection?: string;
  /** by creator id, for the vertex groups of the Myzel meshes that the creator made */
  creators: Map<number, HighlightMode>;
};

/** The paths, the instances and the tissues that a highlight leaves visible */
export function visibleParts(highlight: PartHighlight) {
  const visible = (modes: PartHighlight['paths']) =>
    new Set(
      [...modes]
        .filter(([, mode]) => mode !== HIGHLIGHT.hidden && mode !== HIGHLIGHT.dimmed)
        .map(([id]) => id)
    );
  return {
    paths: visible(highlight.paths),
    instances: visible(highlight.instances),
    creators: visible(highlight.creators)
  };
}

/** The groups of a Myzel mesh that can be told apart, groups above share the mode of the last */
export const MAX_GROUPS = 128;

/**
 * The mode of every vertex group of the Myzel mesh that is drawn, read by the materials that
 * highlight by group and by the id pass, which leaves out what the local view hides
 */
export const groupModes = { value: new Float32Array(MAX_GROUPS) };

/**
 * Sets the mode of every group of a mesh whose groups were made by `creators`. Without a
 * highlight every group is shown as it is.
 */
export function setGroupModes(
  creators: Int32Array | undefined,
  highlight: PartHighlight | undefined
) {
  const modes = groupModes.value;
  if (!highlight || !creators) {
    modes.fill(highlight?.rest ?? HIGHLIGHT.none);
    return;
  }
  for (let group = 0; group < MAX_GROUPS; group++) {
    const creator = creators[Math.min(group, creators.length - 1)];
    modes[group] = highlight.creators.get(creator) ?? highlight.rest;
  }
}

// shared by every highlighting material
const hoverTint = { value: new Color('#4cc2ff') };
const selectedTint = { value: new Color('#ff8a3d') };
const selectedStrength = { value: 1 };

/** How much of the tint of the selection shows, from 0 to 1 */
export function setSelectedStrength(strength: number) {
  selectedStrength.value = strength;
}

/** The tints in the colours of the display, they are mixed in after tone mapping */
export function setHighlightColors(hover: string, selected: string) {
  hoverTint.value.set(hover);
  selectedTint.value.set(selected);
  // `Color.set` converts to linear, the shader mixes them into colours that are sRGB already
  hoverTint.value.convertLinearToSRGB();
  selectedTint.value.convertLinearToSRGB();
}
setHighlightColors('#4cc2ff', '#ff8a3d');

const VERTEX_DECLARATIONS = /* glsl */ `
  uniform float uHighlight;
  #ifdef USE_INSTANCING
    attribute float instanceHighlight;
  #endif
  flat varying float vHighlight;
`;

const VERTEX_MAIN = /* glsl */ `
  vHighlight = uHighlight;
  #ifdef USE_INSTANCING
    vHighlight = instanceHighlight;
  #endif
`;

const GROUP_VERTEX_MAIN = /* glsl */ `
  vHighlight = uGroupModes[int(clamp(group + 0.5, 0.0, ${MAX_GROUPS - 1}.0))];
`;

const FRAGMENT_DECLARATIONS = /* glsl */ `
  uniform vec3 uHoverTint;
  uniform vec3 uSelectedTint;
  uniform float uSelectedStrength;
  flat varying float vHighlight;
`;

// the shading stays visible under the tint, so the shape of the part can still be read
const FRAGMENT_END = /* glsl */ `
  if (vHighlight > 4.5) discard;
  float highlightLuma = dot(gl_FragColor.rgb, vec3(0.299, 0.587, 0.114));
  if (vHighlight > 3.5) {
    gl_FragColor.rgb = mix(gl_FragColor.rgb, vec3(highlightLuma), 0.8) * 0.3;
  } else if (vHighlight > 2.5) {
    gl_FragColor.rgb = mix(
      gl_FragColor.rgb,
      uSelectedTint * (0.45 + 0.75 * highlightLuma),
      0.85 * uSelectedStrength
    );
  } else if (vHighlight > 1.5) {
    gl_FragColor.rgb = mix(
      gl_FragColor.rgb,
      uSelectedTint * (0.45 + 0.75 * highlightLuma),
      0.45 * uSelectedStrength
    );
  } else if (vHighlight > 0.5) {
    gl_FragColor.rgb = mix(gl_FragColor.rgb, uHoverTint * (0.45 + 0.75 * highlightLuma), 0.7);
  }
`;

const MAIN = /void\s+main\s*\(\s*\)\s*\{/;

function beforeMain(source: string, code: string) {
  return source.replace(MAIN, (main) => `${code}\n${main}`);
}

function startOfMain(source: string, code: string) {
  return source.replace(MAIN, (main) => `${main}\n${code}`);
}

/** `main` is the last function of every shader of the viewer, so its end is the last brace */
function endOfMain(source: string, code: string) {
  const end = source.lastIndexOf('}');
  return `${source.slice(0, end)}${code}\n${source.slice(end)}`;
}

const variants = new WeakMap<Material, Map<string, Material>>();

/**
 * `material` showing everything drawn with it in `mode`, instances in the mode of their
 * `instanceHighlight` attribute. The same pattern as `withVertexColors`: a clone whose shader gets
 * a chunk more, the variants of a material share one program and differ in a uniform.
 * With `byGroup` every vertex is in the mode of its group, see `setGroupModes`, the geometry
 * needs the `group` attribute for it.
 */
export function withHighlight(material: Material, mode: HighlightMode, byGroup = false) {
  let modes = variants.get(material);
  if (!modes) variants.set(material, modes = new Map());
  const key = `${mode}:${byGroup}`;
  let variant = modes.get(key);
  if (variant) return variant;

  variant = material.clone();
  const uniforms = {
    uHighlight: { value: mode },
    uHoverTint: hoverTint,
    uSelectedTint: selectedTint,
    uSelectedStrength: selectedStrength,
    uGroupModes: groupModes
  };
  if (variant instanceof ShaderMaterial && material instanceof ShaderMaterial) {
    // the uniforms of the material stay shared, the height range is set on the material itself
    variant.uniforms = { ...material.uniforms, ...uniforms };
  }
  variant.onBeforeCompile = (shader, renderer) => {
    material.onBeforeCompile(shader, renderer);
    Object.assign(shader.uniforms, uniforms);
    // the material that shows the groups reads the attribute already
    const group = byGroup && !/attribute\s+float\s+group\s*;/.test(shader.vertexShader)
      ? 'attribute float group;'
      : '';
    const groups = byGroup ? `uniform float uGroupModes[${MAX_GROUPS}];${group}` : '';
    shader.vertexShader = startOfMain(
      beforeMain(shader.vertexShader, VERTEX_DECLARATIONS + groups),
      byGroup ? VERTEX_MAIN + GROUP_VERTEX_MAIN : VERTEX_MAIN
    );
    shader.fragmentShader = endOfMain(
      beforeMain(shader.fragmentShader, FRAGMENT_DECLARATIONS),
      FRAGMENT_END
    );
  };
  variant.customProgramCacheKey = () => `${material.customProgramCacheKey()}:highlight:${byGroup}`;
  modes.set(key, variant);
  return variant;
}
