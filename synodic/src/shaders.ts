// Define shader material (place this outside the class or in a separate file)
export const shipVertexShader = `
  varying vec2 vUv;
  void main() {
    vUv = uv;
    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
  }
`;

export const shipFragmentShader = `
  uniform sampler2D map;
  uniform float brightness;
  varying vec2 vUv;
  
  void main() {
    vec4 texColor = texture2D(map, vUv);
    gl_FragColor = vec4(texColor.rgb * brightness, texColor.a);
  }
`;
