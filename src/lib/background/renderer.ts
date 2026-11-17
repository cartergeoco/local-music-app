import type { ArtistStore } from '../store';
import { railState } from '../rail/state';
import { cardScale, focusAmount } from '../rail/layout';
import { clamp } from '../rail/physics';
import { FRAGMENT, VERTEX } from './shaders';

/** Drawing buffer resolution relative to device pixels; the gradient is soft enough to upscale. */
export const RENDER_SCALE = 0.5;
const DIM = 0.38;
const NEUTRAL = new Float32Array(15).fill(0.12);

type UniformName =
  | 'u_res'
  | 'u_scale'
  | 'u_time'
  | 'u_palA'
  | 'u_palB'
  | 'u_rectA'
  | 'u_rectB'
  | 'u_radius'
  | 'u_focus'
  | 'u_mix'
  | 'u_dim';

const UNIFORMS: UniformName[] = [
  'u_res',
  'u_scale',
  'u_time',
  'u_palA',
  'u_palB',
  'u_rectA',
  'u_rectB',
  'u_radius',
  'u_focus',
  'u_mix',
  'u_dim',
];

interface GlResources {
  program: WebGLProgram;
  vao: WebGLVertexArrayObject;
  u: Record<UniformName, WebGLUniformLocation | null>;
}

function compile(gl: WebGL2RenderingContext, type: number, source: string): WebGLShader {
  const shader = gl.createShader(type)!;
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS) && !gl.isContextLost()) {
    const log = gl.getShaderInfoLog(shader);
    gl.deleteShader(shader);
    throw new Error(`Shader compile failed: ${log}`);
  }
  return shader;
}

/** Full-window WebGL2 background driven by `railState`. */
export class BackgroundRenderer {
  private readonly gl: WebGL2RenderingContext;
  private res: GlResources | null = null;
  private cssW = 0;
  private cssH = 0;
  private scale = 1;

  constructor(
    private readonly canvas: HTMLCanvasElement,
    private store: ArtistStore,
  ) {
    const gl = canvas.getContext('webgl2', {
      alpha: false,
      antialias: false,
      depth: false,
      stencil: false,
      premultipliedAlpha: false,
      preserveDrawingBuffer: false,
      powerPreference: 'low-power',
    });
    if (!gl) throw new Error('WebGL2 unavailable');
    this.gl = gl;
    this.init();
  }

  get ready(): boolean {
    return this.res !== null;
  }

  setStore(store: ArtistStore): void {
    this.store = store;
  }

  /** Call after the context is restored. */
  init(): void {
    const gl = this.gl;
    const vs = compile(gl, gl.VERTEX_SHADER, VERTEX);
    const fs = compile(gl, gl.FRAGMENT_SHADER, FRAGMENT);
    const program = gl.createProgram()!;
    gl.attachShader(program, vs);
    gl.attachShader(program, fs);
    gl.linkProgram(program);
    gl.detachShader(program, vs);
    gl.detachShader(program, fs);
    gl.deleteShader(vs);
    gl.deleteShader(fs);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS) && !gl.isContextLost()) {
      const log = gl.getProgramInfoLog(program);
      gl.deleteProgram(program);
      throw new Error(`Program link failed: ${log}`);
    }
    const vao = gl.createVertexArray()!;
    const u = {} as GlResources['u'];
    for (const name of UNIFORMS) u[name] = gl.getUniformLocation(program, name);
    this.res = { program, vao, u };
    if (this.cssW) this.resize(this.cssW, this.cssH, this.scale / RENDER_SCALE);
  }

  /** Context loss invalidates every GL object; drop references without deleting. */
  lose(): void {
    this.res = null;
  }

  resize(cssW: number, cssH: number, devicePixelRatio: number): void {
    this.cssW = cssW;
    this.cssH = cssH;
    this.scale = devicePixelRatio * RENDER_SCALE;
    const w = Math.max(1, Math.round(cssW * this.scale));
    const h = Math.max(1, Math.round(cssH * this.scale));
    if (this.canvas.width !== w || this.canvas.height !== h) {
      this.canvas.width = w;
      this.canvas.height = h;
    }
  }

  render(time: number): void {
    const res = this.res;
    if (!res || this.gl.isContextLost()) return;
    const gl = this.gl;
    const { u } = res;
    const layout = railState.layout;
    const view = railState.view;
    const count = view.length;

    gl.viewport(0, 0, this.canvas.width, this.canvas.height);
    gl.useProgram(res.program);
    gl.bindVertexArray(res.vao);
    gl.uniform2f(u.u_res, this.canvas.width, this.canvas.height);
    gl.uniform1f(u.u_scale, this.canvas.height / Math.max(1, this.cssH));
    gl.uniform1f(u.u_time, time);
    gl.uniform1f(u.u_dim, DIM);

    if (!layout || count === 0) {
      gl.uniform3fv(u.u_palA, NEUTRAL);
      gl.uniform3fv(u.u_palB, NEUTRAL);
      gl.uniform2f(u.u_focus, 0, 0);
      gl.uniform1f(u.u_mix, 0);
    } else {
      const pos = railState.pos;
      const p = clamp(pos, 0, count - 1);
      const i0 = Math.floor(p);
      const i1 = Math.min(i0 + 1, count - 1);
      const palettes = this.store.palettes;
      gl.uniform3fv(u.u_palA, palettes, view[i0] * 15, 15);
      gl.uniform3fv(u.u_palB, palettes, view[i1] * 15, 15);
      gl.uniform1f(u.u_mix, p - i0);

      const fA = focusAmount(i0, pos);
      const fB = i1 === i0 ? 0 : focusAmount(i1, pos);
      const sA = cardScale(fA);
      const sB = cardScale(fB);
      const hw = layout.cardW / 2;
      const hh = layout.cardH / 2;
      gl.uniform4f(u.u_rectA, layout.centerX + (i0 - pos) * layout.pitch, layout.centerY, hw * sA, hh * sA);
      gl.uniform4f(u.u_rectB, layout.centerX + (i1 - pos) * layout.pitch, layout.centerY, hw * sB, hh * sB);
      gl.uniform2f(u.u_radius, layout.radius * sA, layout.radius * sB);
      gl.uniform2f(u.u_focus, fA, fB);
    }

    gl.drawArrays(gl.TRIANGLES, 0, 3);
  }

  destroy(): void {
    const gl = this.gl;
    if (this.res && !gl.isContextLost()) {
      gl.deleteVertexArray(this.res.vao);
      gl.deleteProgram(this.res.program);
    }
    this.res = null;
    gl.getExtension('WEBGL_lose_context')?.loseContext();
    this.canvas.width = 0;
    this.canvas.height = 0;
  }
}
