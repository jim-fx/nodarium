import {
  type Box3,
  Color,
  DoubleSide,
  type Material,
  MeshBasicMaterial,
  MeshMatcapMaterial,
  ShaderMaterial,
  type Texture,
  TextureLoader,
  Vector2,
  Vector3
} from 'three';

const loader = new TextureLoader();
let matcapTexture: Texture | undefined;
export const matcapLoaded = new Promise<Texture>((resolve) => {
  matcapTexture = loader.load('/matcap_green.jpg', resolve);
  matcapTexture.colorSpace = 'srgb';
});

// shared by all shaders, handles plain and instanced meshes
const vertexShader = /* glsl */ `
  uniform vec2 uHeightRange;

  varying vec3 vWorldNormal;
  varying vec3 vWorldPosition;
  varying float vHeight;
  varying float vVariation;

  float hash(vec3 p) {
    return fract(sin(dot(p, vec3(12.9898, 78.233, 37.719))) * 43758.5453);
  }

  void main() {
    vec4 pos = vec4(position, 1.0);
    vec3 n = normal;
    float variation = 0.5;
    #ifdef USE_INSTANCING
      pos = instanceMatrix * pos;
      n = mat3(instanceMatrix) * n;
      // every leaf gets its own slightly different color
      variation = hash(instanceMatrix[3].xyz);
    #endif

    vec4 world = modelMatrix * pos;
    vWorldPosition = world.xyz;
    vWorldNormal = mat3(modelMatrix) * n;
    vHeight = clamp((world.y - uHeightRange.x) / max(uHeightRange.y - uHeightRange.x, 1e-4), 0.0, 1.0);
    vVariation = variation;

    gl_Position = projectionMatrix * viewMatrix * world;
  }
`;

const litFragmentShader = /* glsl */ `
  uniform vec3 uBase;
  uniform vec3 uTip;
  uniform vec3 uTranslucent;
  uniform vec3 uSky;
  uniform vec3 uGround;
  uniform vec3 uLightDirection;
  uniform vec3 uLightColor;
  uniform float uVariation;
  uniform float uTranslucency;

  varying vec3 vWorldNormal;
  varying vec3 vWorldPosition;
  varying float vHeight;
  varying float vVariation;

  void main() {
    vec3 N = normalize(vWorldNormal);
    if (!gl_FrontFacing) N = -N;
    vec3 V = normalize(cameraPosition - vWorldPosition);
    vec3 L = normalize(uLightDirection);
    vec3 H = normalize(L + V);

    // darker and more saturated at the base, lighter towards the tips
    vec3 albedo = mix(uBase, uTip, smoothstep(0.0, 1.0, vHeight));
    albedo *= 1.0 + (vVariation - 0.5) * uVariation;

    // light coming from the sky and bouncing off the ground
    vec3 ambient = mix(uGround, uSky, N.y * 0.5 + 0.5);
    float occlusion = mix(0.5, 1.0, smoothstep(0.0, 0.3, vHeight));

    // wrapped diffuse, leaves are never completely black on the shadow side
    float NdotL = dot(N, L);
    float diffuse = max((NdotL + 0.3) / 1.3, 0.0);

    // light shining through thin leaves when they face away from the sun
    float through = max(-NdotL, 0.0) * (0.3 + 0.7 * pow(max(dot(V, -L), 0.0), 2.0));

    float specular = pow(max(dot(N, H), 0.0), 24.0) * 0.06;
    float rim = pow(1.0 - max(dot(N, V), 0.0), 3.0) * 0.2;

    vec3 color = albedo * (ambient * occlusion + diffuse * uLightColor)
      + uTranslucent * through * uTranslucency
      + (specular + rim) * uSky;

    gl_FragColor = vec4(color, 1.0);
    #include <tonemapping_fragment>
    #include <colorspace_fragment>
  }
`;

const normalsFragmentShader = /* glsl */ `
  varying vec3 vWorldNormal;

  void main() {
    // the normal as it is stored, not flipped on back faces
    gl_FragColor = vec4(normalize(vWorldNormal) * 0.5 + 0.5, 1.0);
    #include <colorspace_fragment>
  }
`;

const facesFragmentShader = /* glsl */ `
  uniform vec3 uFront;
  uniform vec3 uBack;

  varying vec3 vWorldNormal;
  varying vec3 vWorldPosition;

  void main() {
    vec3 N = normalize(vWorldNormal);
    if (!gl_FrontFacing) N = -N;
    vec3 V = normalize(cameraPosition - vWorldPosition);
    float shade = 0.35 + 0.65 * max(dot(N, V), 0.0);
    gl_FragColor = vec4((gl_FrontFacing ? uFront : uBack) * shade, 1.0);
    #include <colorspace_fragment>
  }
`;

type LitOptions = {
  base: string;
  tip: string;
  translucent: string;
  translucency: number;
  variation: number;
};

function createLitMaterial(options: LitOptions) {
  return new ShaderMaterial({
    vertexShader,
    fragmentShader: litFragmentShader,
    side: DoubleSide,
    uniforms: {
      uHeightRange: { value: new Vector2(-1, 1) },
      uBase: { value: new Color(options.base) },
      uTip: { value: new Color(options.tip) },
      uTranslucent: { value: new Color(options.translucent) },
      uTranslucency: { value: options.translucency },
      uVariation: { value: options.variation },
      uSky: { value: new Color('#cfe6ff').multiplyScalar(0.45) },
      uGround: { value: new Color('#5a4a30').multiplyScalar(0.3) },
      uLightDirection: { value: new Vector3(0.5, 1, 0.3) },
      uLightColor: { value: new Color('#fff1d6').multiplyScalar(0.8) }
    }
  });
}

export const materials = [
  {
    id: 'plant',
    name: 'Plant',
    create: () =>
      createLitMaterial({
        base: '#123d08',
        tip: '#5fa61e',
        translucent: '#9ccf1f',
        translucency: 0.5,
        variation: 0.35
      })
  },
  {
    id: 'clay',
    name: 'Clay',
    create: () =>
      createLitMaterial({
        base: '#8a8580',
        tip: '#c9c4bd',
        translucent: '#000000',
        translucency: 0,
        variation: 0
      })
  },
  {
    id: 'matcap',
    name: 'Matcap',
    create: () => new MeshMatcapMaterial({ color: 0xffffff, matcap: matcapTexture, side: DoubleSide })
  },
  {
    id: 'wireframe',
    name: 'Wireframe',
    create: () => new MeshBasicMaterial({ color: 0x8cc63f, wireframe: true, side: DoubleSide })
  },
  {
    id: 'normals',
    name: 'Normals',
    create: () =>
      new ShaderMaterial({
        vertexShader,
        fragmentShader: normalsFragmentShader,
        side: DoubleSide,
        uniforms: { uHeightRange: { value: new Vector2(-1, 1) } }
      })
  },
  {
    id: 'faces',
    name: 'Faces',
    create: () =>
      new ShaderMaterial({
        vertexShader,
        fragmentShader: facesFragmentShader,
        side: DoubleSide,
        uniforms: {
          uHeightRange: { value: new Vector2(-1, 1) },
          uFront: { value: new Color('#3b82f6') },
          uBack: { value: new Color('#ef4444') }
        }
      })
  }
] as const satisfies { id: string; name: string; create: () => Material }[];

export type MaterialId = (typeof materials)[number]['id'];

export function getMaterial(id: string) {
  return materials.find((m) => m.id === id) ?? materials[0];
}

/** the lit materials fade from base to tip over the height of the whole result */
export function setHeightRange(material: Material, box: Box3) {
  if (!(material instanceof ShaderMaterial) || box.isEmpty()) return;
  material.uniforms.uHeightRange?.value.set(box.min.y, box.max.y);
}
