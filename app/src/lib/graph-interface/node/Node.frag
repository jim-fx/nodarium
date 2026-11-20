varying vec2 vUv;

// in graph units, 10 px of the HTML node
uniform float uWidth;
uniform float uHeight;
// graph units per screen pixel
uniform float uPixel;

uniform vec3 uColorDark;
uniform vec3 uColorBright;
uniform vec3 uStrokeColor;
uniform vec3 uBarColor;
uniform bool uHasBar;
uniform bool uIsMuted;

const float HEADER_HEIGHT = 5.0;
const float RADIUS = 0.5;
uniform float uSectionHeights[16];
uniform int uNumSections;
// bit 0 is the header, bit i + 1 the section i
uniform int uLeftSockets;
uniform int uRightSockets;
uniform int uPossibleSockets;

// The notches of NodeHeader, NodeParameter and NodeOutput are cubic curves; an ellipse with
// three quarters of their depth and half their height is within 3 % of them.
const vec2 HEADER_NOTCH = vec2(0.825, 0.85);
const vec2 ROW_NOTCH = vec2(0.9, 1.0);
const vec2 ROW_NOTCH_POSSIBLE = vec2(1.05, 1.1);

float sdRoundBox(vec2 p, vec2 b, float r) {
  vec2 q = abs(p) - b + r;
  return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r;
}

float sdEllipse(vec2 p, vec2 r) {
  float k0 = length(p / r);
  float k1 = length(p / (r * r));
  return k0 * (k0 - 1.0) / k1;
}

bool hasBit(int mask, int bit) {
  return ((mask >> bit) & 1) == 1;
}

float notch(float d, vec2 p, float inset, int bit, float centerY, vec2 radius) {
  if (hasBit(uLeftSockets, bit)) {
    d = max(d, -sdEllipse(p - vec2(inset, centerY), radius));
  }
  if (hasBit(uRightSockets, bit)) {
    d = max(d, -sdEllipse(p - vec2(uWidth - inset, centerY), radius));
  }
  return d;
}

void main() {
  vec2 p = vec2(vUv.x * uWidth, (1.0 - vUv.y) * uHeight);

  // the HTML outline is 2px of the node; zoomed out it stays one screen pixel wide
  float halfStroke = max(0.1, 0.5 * uPixel);
  float aa = uPixel;
  // the outline is centred on the path, its outer edge is the edge of the node
  float inset = halfStroke;

  vec2 size = vec2(uWidth, uHeight);
  float d = sdRoundBox(p - size * 0.5, size * 0.5 - inset, RADIUS);

  // a notch is smaller than its row, so only the row of the fragment can cut into it
  bool nearSide = p.x < 2.0 || p.x > uWidth - 2.0;
  float divider = abs(p.y - HEADER_HEIGHT);
  if (p.y < HEADER_HEIGHT) {
    if (nearSide) d = notch(d, p, inset, 0, HEADER_HEIGHT * 0.5, HEADER_NOTCH);
  } else {
    float top = HEADER_HEIGHT;
    for (int i = 0; i < 16; i++) {
      if (i >= uNumSections) break;
      float h = uSectionHeights[i];
      if (p.y < top + h || i == uNumSections - 1) {
        if (i < uNumSections - 1) divider = min(divider, top + h - p.y);
        divider = min(divider, p.y - top);
        if (nearSide) {
          vec2 radius = hasBit(uPossibleSockets, i + 1) ? ROW_NOTCH_POSSIBLE : ROW_NOTCH;
          d = notch(d, p, inset, i + 1, top + h * 0.5, radius);
        }
        break;
      }
      top += h;
    }
  }

  float coverage = 1.0 - smoothstep(-0.5 * aa, 0.5 * aa, d - halfStroke);
  if (coverage <= 0.0) discard;

  vec3 fill = p.y < HEADER_HEIGHT ? uColorBright : uColorDark;
  if (uHasBar && p.y < HEADER_HEIGHT) {
    float bar = abs(p.x - 2.0 * halfStroke - 0.15) - 0.15;
    fill = mix(uBarColor, fill, smoothstep(-0.5 * aa, 0.5 * aa, bar));
  }

  float stroke = smoothstep(-0.5 * aa, 0.5 * aa, d + halfStroke);
  stroke = max(stroke, 1.0 - smoothstep(-0.5 * aa, 0.5 * aa, divider - halfStroke));

  float alpha = coverage * (uIsMuted ? 0.5 : 1.0);
  gl_FragColor = vec4(mix(fill, uStrokeColor, stroke), alpha);
}
