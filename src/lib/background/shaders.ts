export const VERTEX = /* glsl */ `#version 300 es
void main() {
  // Single full-screen triangle; no vertex buffers needed.
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}`;

export const FRAGMENT = /* glsl */ `#version 300 es
precision highp float;

uniform vec2 u_res;        // drawing buffer size, px
uniform float u_scale;     // buffer px per CSS px
uniform float u_time;
uniform vec3 u_palA[5];
uniform vec3 u_palB[5];
uniform vec4 u_rectA;      // center x, center y, half width, half height (CSS px)
uniform vec4 u_rectB;
uniform vec2 u_radius;     // corner radius for A, B (CSS px)
uniform vec2 u_focus;      // focus amount for A, B
uniform float u_mix;       // background blend from A to B
uniform float u_dim;       // background brightness outside the focused card

out vec4 outColor;

float hash(vec2 p) {
  p = fract(p * vec2(123.34, 456.21));
  p += dot(p, p + 45.32);
  return fract(p.x * p.y);
}

float noise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1, 0)), u.x),
             mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), u.x), u.y);
}

float fbm(vec2 p) {
  float v = 0.0;
  float a = 0.5;
  for (int i = 0; i < 3; i++) {
    v += a * noise(p);
    p = p * 2.03 + 17.1;
    a *= 0.5;
  }
  return v;
}

vec3 rampA(float t) {
  t = clamp(t, 0.0, 1.0) * 4.0;
  int i = int(min(floor(t), 3.0));
  return mix(u_palA[i], u_palA[i + 1], smoothstep(0.0, 1.0, t - float(i)));
}

vec3 rampB(float t) {
  t = clamp(t, 0.0, 1.0) * 4.0;
  int i = int(min(floor(t), 3.0));
  return mix(u_palB[i], u_palB[i + 1], smoothstep(0.0, 1.0, t - float(i)));
}

float roundedRectMask(vec2 p, vec4 rect, float r) {
  vec2 q = abs(p - rect.xy) - rect.zw + r;
  float d = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
  return 1.0 - smoothstep(-1.0, 1.0, d);
}

void main() {
  vec2 css = vec2(gl_FragCoord.x, u_res.y - gl_FragCoord.y) / u_scale;
  vec2 uv = css / (u_res.y / u_scale) * 1.7;
  float t = u_time * 0.06;

  vec2 q = vec2(fbm(uv + vec2(0.0, t)), fbm(uv + vec2(5.2, 1.3) - t));
  float v = fbm(uv + 1.8 * q + vec2(1.7, 9.2) + 0.12 * t);
  v = smoothstep(0.15, 0.85, v);

  vec3 a = rampA(v);
  vec3 b = rampB(v);

  vec2 n = css / (u_res / u_scale) - 0.5;
  float vignette = 1.0 - dot(n, n) * 0.9;
  vec3 color = mix(a, b, u_mix) * u_dim * vignette;

  color = mix(color, a, roundedRectMask(css, u_rectA, u_radius.x) * u_focus.x);
  color = mix(color, b, roundedRectMask(css, u_rectB, u_radius.y) * u_focus.y);

  // Dither to hide 8-bit banding in slow gradients.
  color += (hash(gl_FragCoord.xy + fract(u_time)) - 0.5) / 255.0;
  outColor = vec4(color, 1.0);
}`;
