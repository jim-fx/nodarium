precision highp float;
// For WebGL1 make sure this extension is enabled in your material:
// #extension GL_OES_standard_derivatives : enable

varying vec2 vUv;

const float PI = 3.14159265359;

uniform vec2 dimensions;
uniform vec3 camPos;
uniform vec2 zoomLimits;
uniform vec3 backgroundColor;
uniform vec3 lineColor;
uniform int gridType; // 0 = grid lines, 1 = dots

// Anti-aliased step: threshold in the same units as `value`
float aaStep(float threshold, float value, float deriv) {
    float w = deriv * 0.5; // ~one pixel
    return smoothstep(threshold - w, threshold + w, value);
}

float grid(float x, float y, float divisions, float thickness) {
    // Continuous grid coordinates
    float gx = x * divisions;
    float gy = y * divisions;

    // Distance to nearest grid line (0 at the line)
    float fx = fract(gx);
    fx = min(fx, 1.0 - fx);
    float fy = fract(gy);
    fy = min(fy, 1.0 - fy);

    // Derivatives in screen space – use the continuous coords here
    float dx = fwidth(gx);
    float dy = fwidth(gy);

    // Keep the original semantics: thickness is the threshold in the [0, 0.5] distance domain
    float lineX = 1.0 - aaStep(thickness, fx, dx);
    float lineY = 1.0 - aaStep(thickness, fy, dy);

    return clamp(lineX + lineY, 0.0, 1.0);
}

float circle_grid(float x, float y, float divisions, float circleRadius) {
    float gridX = mod(x + divisions * 0.5, divisions) - divisions * 0.5;
    float gridY = mod(y + divisions * 0.5, divisions) - divisions * 0.5;

    vec2 g = vec2(gridX, gridY);
    float d = length(g);

    // Screen-space derivative for AA on the circle edge
    float w = fwidth(d);

    float circle = 1.0 - smoothstep(circleRadius - w, circleRadius + w, d);
    return circle;
}

float lerp(float a, float b,float t) {
    return a * (1.0 - t) + b * t;
}

void main(void) {

    float cx = camPos.x;
    float cy = camPos.y;
    float cz = camPos.z;

    float width = dimensions.x;
    float height =  dimensions.y;

    float minZ = zoomLimits.x;
    float maxZ = zoomLimits.y;

    float divisions = 0.1 / cz;
    float thickness = 0.05 / cz;

    float nz = (cz - minZ) / (maxZ - minZ);

    float ux = (vUv.x - 0.5) * width + cx * cz;
    float uy = (vUv.y - 0.5) * height - cy * cz;

    if(gridType == 0) {
      // extra small grid
      float m1 = grid(ux, uy, divisions * 4.0,  thickness * 4.0)  * 0.9;
      float m2 = grid(ux, uy, divisions * 16.0, thickness * 16.0) * 0.5;
      float xsmall = max(m1, m2);
      
      float s3 = circle_grid(ux, uy, cz / 1.6, 1.0) * 0.5;
      xsmall = max(xsmall, s3);

      // small grid
      float c1 = grid(ux, uy, divisions,        thickness)        * 0.6;
      float c2 = grid(ux, uy, divisions * 2.0,  thickness * 2.0)  * 0.5;
      float small = max(c1, c2);

      float s1 = circle_grid(ux, uy, cz * 10.0, 2.0) * 0.5;
      small = max(small, s1);

      // large grid
      float c3 = grid(ux, uy, divisions / 8.0,  thickness / 8.0)  * 0.5;
      float c4 = grid(ux, uy, divisions / 2.0,  thickness / 4.0)  * 0.4;
      float large = max(c3, c4);

      float s2 = circle_grid(ux, uy, cz * 20.0, 1.0) * 0.4;
      large = max(large, s2);

      float c = mix(large, small, min(nz * 2.0 + 0.05, 1.0));
      c = mix(c, xsmall, clamp((nz - 0.3) / 0.7, 0.0, 1.0));

      vec3 color = mix(backgroundColor, lineColor, c); 

      gl_FragColor = vec4(color, 1.0);
    } else {
      float large = circle_grid(ux, uy, cz * 20.0, 1.0) * 0.4;

      float medium = circle_grid(ux, uy, cz * 10.0, 1.0) * 0.6;

      float small = circle_grid(ux, uy, cz * 2.5, 1.0) * 0.8;

      float c = mix(large, medium, min(nz * 2.0 + 0.05, 1.0));
      c = mix(c, small, clamp((nz - 0.3) / 0.7, 0.0, 1.0));

      vec3 color = mix(backgroundColor, lineColor, c); 

      gl_FragColor = vec4(color, 1.0);
    }

}

