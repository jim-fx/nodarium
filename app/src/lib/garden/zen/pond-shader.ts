// Shader for the Reflector of the pond. Kept inline instead of in .frag/.vert files,
// vite-plugin-glsl would try to resolve the #include chunks of three.js.

export const pondVertexShader = /* glsl */ `
  uniform mat4 textureMatrix;
  varying vec4 vUv;
  varying vec3 vWorldPosition;

  #include <common>
  #include <fog_pars_vertex>
  #include <logdepthbuf_pars_vertex>

  void main() {
    vUv = textureMatrix * vec4(position, 1.0);
    vec4 worldPosition = modelMatrix * vec4(position, 1.0);
    vWorldPosition = worldPosition.xyz;
    vec4 mvPosition = viewMatrix * worldPosition;
    gl_Position = projectionMatrix * mvPosition;

    #include <logdepthbuf_vertex>
    #include <fog_vertex>
  }
`;

export const pondFragmentShader = /* glsl */ `
  uniform vec3 color;
  uniform sampler2D tDiffuse;
  uniform float time;
  uniform vec2 pondCenter;
  uniform float pondRadius;
  varying vec4 vUv;
  varying vec3 vWorldPosition;

  #include <common>
  #include <fog_pars_fragment>
  #include <logdepthbuf_pars_fragment>

  const float PERIOD = 11.0;
  const int DROPS = 3;

  vec2 hash2(float n) {
    return fract(sin(vec2(n, n + 1.7)) * vec2(43758.5453, 22578.1459));
  }

  void main() {
    #include <logdepthbuf_fragment>

    // a few drops at a time, each one spreads a ring of ripples and fades
    vec2 offset = vec2(0.0);
    for (int i = 0; i < DROPS; i++) {
      float shifted = time + float(i) * PERIOD / float(DROPS);
      float age = mod(shifted, PERIOD);
      float cycle = floor(shifted / PERIOD) * 7.0 + float(i) * 13.0;
      vec2 origin = pondCenter + (hash2(cycle) - 0.5) * pondRadius * 1.1;

      vec2 d = vWorldPosition.xz - origin;
      float dist = length(d) + 1e-4;
      float front = age * 0.32;
      float envelope = exp(-abs(dist - front) * 7.0)
        * smoothstep(0.0, 0.4, age)
        * (1.0 - age / PERIOD)
        / (1.0 + dist * 2.0);
      offset += (d / dist) * sin((dist - front) * 34.0) * envelope;
    }

    vec4 uv = vUv;
    uv.xy += offset * 0.035 * uv.w;
    vec3 reflection = texture2DProj(tDiffuse, uv).rgb;

    vec3 viewDirection = normalize(cameraPosition - vWorldPosition);
    float fresnel = pow(1.0 - clamp(viewDirection.y, 0.0, 1.0), 2.0);
    // there is no sky in the scene, a faint overcast tone keeps the surface readable as water
    vec3 sky = mix(vec3(0.05, 0.055, 0.06), vec3(0.22, 0.235, 0.25), fresnel);
    vec3 water = mix(color, reflection + sky, 0.25 + 0.6 * fresnel);
    water += length(offset) * 0.015;

    gl_FragColor = vec4(water, 1.0);

    #include <tonemapping_fragment>
    #include <colorspace_fragment>
    #include <fog_fragment>
  }
`;
