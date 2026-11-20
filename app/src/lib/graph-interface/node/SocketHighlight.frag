varying vec2 vUv;

uniform vec3 uColor;
uniform float uOpacity;

// the circle of .possible-socket in the HTML: 30px wide with a 10px box-shadow
const float RADIUS = 1.5;
const float GLOW = 1.0;
const float SIZE = 2.5;

void main() {
  float r = length(vUv - 0.5) * 2.0 * SIZE;
  float circle = 1.0 - smoothstep(RADIUS - 0.05, RADIUS + 0.05, r);
  float glow = 0.5 * (1.0 - smoothstep(RADIUS, RADIUS + GLOW, r));
  float alpha = max(circle, glow) * uOpacity;
  if (alpha <= 0.0) discard;
  gl_FragColor = vec4(uColor, alpha);
}
