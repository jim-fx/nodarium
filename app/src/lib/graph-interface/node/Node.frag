varying vec2 vUv;

uniform float uWidth;
uniform float uHeight;
uniform float uZoom; 

uniform vec3 uColorDark;
uniform vec3 uColorBright;
uniform vec3 uStrokeColor;

const float uHeaderHeight = 5.0; 
uniform float uSectionHeights[16]; 
uniform int uNumSections;

float msign(in float x) { return (x < 0.0) ? -1.0 : 1.0; }
float sdCircle(vec2 p, float r) { return length(p) - r; }

vec4 roundedBoxSDF( in vec2 p, in vec2 b, in float r, in float s) {
  vec2 q = abs(p) - b + r;
  float l = b.x + b.y + 1.570796 * r;
  float k1 = min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r;
  float k2 = ((q.x > 0.0) ? atan(q.y, q.x) : 1.570796);
  float k3 = 3.0 + 2.0 * msign(min(p.x, -p.y)) - msign(p.x);
  float k4 = msign(p.x * p.y);
  float k5 = r * k2 + max(-q.x, 0.0);
  float ra = s * round(k1 / s);
  float l2 = l + 1.570796 * ra;
  return vec4(k1 - ra, k3 * l2 + k4 * (b.y + ((q.y > 0.0) ? k5 + k2 * ra : q.y)), 4.0 * l2, k1);
}

void main(){
  float strokeWidth = mix(2.0, 0.5, uZoom);
  
  float borderRadius = 0.5;
  float dentRadius = 0.8;

  float y = (1.0 - vUv.y) * uHeight;
  float x = vUv.x * uWidth;
  vec2 size = vec2(uWidth, uHeight);
  vec2 uvCenter = (vUv - 0.5) * 2.0;

  vec4 boxData = roundedBoxSDF(uvCenter * size, size, borderRadius * 2.0, 0.0);
  float sceneSDF = boxData.w;

  vec2 headerDentPos = vec2(uWidth, uHeaderHeight * 0.5);
  float headerDentDist = sdCircle(vec2(x, y) - headerDentPos, dentRadius);
  sceneSDF = max(sceneSDF, -headerDentDist*2.0);

  float currentYBoundary = uHeaderHeight;
  float previousYBoundary = uHeaderHeight;

  for (int i = 0; i < 16; i++) {
    if (i >= uNumSections) break;
    
    float sectionHeight = uSectionHeights[i];
    currentYBoundary += sectionHeight;
    
    float centerY = previousYBoundary + (sectionHeight * 0.5);
    vec2 circlePos = vec2(0.0, centerY);
    float circleDist = sdCircle(vec2(x, y) - circlePos, dentRadius);
    
    sceneSDF = max(sceneSDF, -circleDist*2.0);
    previousYBoundary = currentYBoundary;
  }

  if (sceneSDF > 0.05) { 
    gl_FragColor = vec4(0.0, 0.0, 0.0, 0.0);
    return;
  }

  vec3 finalColor = (y < uHeaderHeight) ? uColorBright : uColorDark;
  bool isDivider = false;

  float dividerY = uHeaderHeight;
  if (abs(y - dividerY) < strokeWidth * 0.25) isDivider = true;
  
  for (int i = 0; i < 16; i++) {
    if (i >= uNumSections - 1) break;
    dividerY += uSectionHeights[i];
    if (abs(y - dividerY) < strokeWidth * 0.25) isDivider = true;
  }

  if (sceneSDF > -strokeWidth || isDivider) {
    gl_FragColor = vec4(uStrokeColor, 1.0);
  } else {
    gl_FragColor = vec4(finalColor, 1.0);
  }
}
