varying vec2 vUv;

// in graph units
uniform vec2 uSize;
// graph units per screen pixel
uniform float uPixel;

uniform vec3 uColor;
uniform vec3 uStrokeColor;
uniform float uStrokeAlpha;
// in screen pixels
uniform float uStrokeWidth;

const float RADIUS = 0.5;
const float FILL_ALPHA = 0.14;

float sdRoundBox(vec2 p, vec2 b, float r) {
  vec2 q = abs(p) - b + r;
  return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r;
}

void main() {
  vec2 p = (vUv - 0.5) * uSize;
  float aa = uPixel;
  float d = sdRoundBox(p, uSize * 0.5, RADIUS);

  float coverage = 1.0 - smoothstep(-0.5 * aa, 0.5 * aa, d);
  if (coverage <= 0.0) discard;

  float stroke = 1.0 - smoothstep(-0.5 * aa, 0.5 * aa, -d - uStrokeWidth * uPixel);
  vec3 color = mix(uColor, uStrokeColor, stroke);
  float alpha = mix(FILL_ALPHA, uStrokeAlpha, stroke);
  gl_FragColor = vec4(color, alpha * coverage);
}
