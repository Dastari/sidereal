// Grid vertex shader - renders infinite grid with multi-level subdivision
export const gridVertexShader = `#version 300 es
in vec2 a_position;
out vec2 v_worldPos;

uniform vec2 u_resolution;
uniform vec2 u_camera;
uniform float u_zoom;

void main() {
  // Convert screen position to world position
  vec2 screenPos = a_position * u_resolution;
  vec2 worldPos = screenPos / u_zoom + u_camera - (u_resolution / u_zoom) * 0.5;
  v_worldPos = worldPos;
  gl_Position = vec4(a_position * 2.0 - 1.0, 0.0, 1.0);
}
`

export const gridFragmentShader = `#version 300 es
precision highp float;

in vec2 v_worldPos;
out vec4 fragColor;

uniform float u_zoom;
uniform vec3 u_gridMajor;
uniform vec3 u_gridMinor;
uniform vec3 u_gridMicro;
uniform vec3 u_background;

float gridLine(float coord, float spacing, float lineWidth) {
  float halfWidth = lineWidth * 0.5;
  float d = abs(mod(coord + halfWidth, spacing) - halfWidth);
  return smoothstep(lineWidth, 0.0, d);
}

void main() {
  // Calculate appropriate grid spacing based on zoom level
  // Major grid at 1000m, 100m subdivisions, 10m micro
  float majorSpacing = 1000.0;
  float minorSpacing = 100.0;
  float microSpacing = 10.0;
  
  // Line widths in world units (adjusted by zoom)
  float invZoom = 1.0 / u_zoom;
  float majorWidth = 2.0 * invZoom;
  float minorWidth = 1.0 * invZoom;
  float microWidth = 0.5 * invZoom;
  
  // Calculate grid lines
  float majorX = gridLine(v_worldPos.x, majorSpacing, majorWidth);
  float majorY = gridLine(v_worldPos.y, majorSpacing, majorWidth);
  float major = max(majorX, majorY);
  
  float minorX = gridLine(v_worldPos.x, minorSpacing, minorWidth);
  float minorY = gridLine(v_worldPos.y, minorSpacing, minorWidth);
  float minor = max(minorX, minorY);
  
  // Only show micro grid when zoomed in enough
  float microFade = smoothstep(0.5, 2.0, u_zoom);
  float microX = gridLine(v_worldPos.x, microSpacing, microWidth);
  float microY = gridLine(v_worldPos.y, microSpacing, microWidth);
  float micro = max(microX, microY) * microFade;
  
  // Composite colors
  vec3 color = u_background;
  color = mix(color, u_gridMicro, micro * 0.6);
  color = mix(color, u_gridMinor, minor * 0.8);
  color = mix(color, u_gridMajor, major);
  
  // Origin lines (thicker, different color)
  float originWidth = 3.0 * invZoom;
  float originX = gridLine(v_worldPos.x, 100000.0, originWidth);
  float originY = gridLine(v_worldPos.y, 100000.0, originWidth);
  float origin = max(originX, originY);
  color = mix(color, vec3(0.4, 0.5, 0.7), origin * 0.8);
  
  fragColor = vec4(color, 1.0);
}
`

// Node/Entity vertex shader
export const nodeVertexShader = `#version 300 es
in vec2 a_position;
in vec3 a_color;
in float a_size;
in float a_selected;
in float a_hovered;

uniform vec2 u_resolution;
uniform vec2 u_camera;
uniform float u_zoom;

out vec3 v_color;
out float v_selected;
out float v_hovered;

void main() {
  vec2 worldOffset = a_position - u_camera;
  vec2 screenPos = worldOffset * u_zoom;
  vec2 clipPos = screenPos / (u_resolution * 0.5);
  
  gl_Position = vec4(clipPos.x, clipPos.y, 0.0, 1.0);
  gl_PointSize = a_size * u_zoom * 0.15 + 8.0;
  
  v_color = a_color;
  v_selected = a_selected;
  v_hovered = a_hovered;
}
`

export const nodeFragmentShader = `#version 300 es
precision mediump float;

in vec3 v_color;
in float v_selected;
in float v_hovered;
out vec4 fragColor;

void main() {
  vec2 center = gl_PointCoord - vec2(0.5);
  float dist = length(center);
  
  // Anti-aliased circle
  float alpha = 1.0 - smoothstep(0.35, 0.5, dist);
  if (alpha < 0.01) discard;
  
  vec3 color = v_color;
  
  // Selection ring
  if (v_selected > 0.5) {
    float ringDist = abs(dist - 0.4);
    float ring = 1.0 - smoothstep(0.0, 0.08, ringDist);
    color = mix(color, vec3(0.4, 0.95, 0.6), ring);
  }
  
  // Hover glow
  if (v_hovered > 0.5 && v_selected < 0.5) {
    float glow = 1.0 - smoothstep(0.3, 0.5, dist);
    color = mix(color, vec3(1.0, 1.0, 1.0), glow * 0.3);
  }
  
  fragColor = vec4(color, alpha);
}
`

// Edge vertex shader
export const edgeVertexShader = `#version 300 es
in vec2 a_position;
in vec3 a_color;

uniform vec2 u_resolution;
uniform vec2 u_camera;
uniform float u_zoom;

out vec3 v_color;

void main() {
  vec2 worldOffset = a_position - u_camera;
  vec2 screenPos = worldOffset * u_zoom;
  vec2 clipPos = screenPos / (u_resolution * 0.5);
  
  gl_Position = vec4(clipPos.x, clipPos.y, 0.0, 1.0);
  v_color = a_color;
}
`

export const edgeFragmentShader = `#version 300 es
precision mediump float;

in vec3 v_color;
out vec4 fragColor;

void main() {
  fragColor = vec4(v_color, 0.6);
}
`
