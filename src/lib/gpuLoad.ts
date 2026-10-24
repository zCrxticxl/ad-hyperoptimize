/** Puts load on the GPU for `ms` with a heavy WebGL fragment shader, so the
 * GPU leaves its idle link state while the PCIe link is sampled. */
export function gpuLoad(ms: number): Promise<void> {
  return new Promise((resolve) => {
    const canvas = document.createElement("canvas");
    canvas.width = 2048;
    canvas.height = 2048;
    const gl = canvas.getContext("webgl", { powerPreference: "high-performance" });
    if (!gl) return resolve();
    const sh = (type: number, src: string) => {
      const s = gl.createShader(type)!;
      gl.shaderSource(s, src);
      gl.compileShader(s);
      return s;
    };
    const prog = gl.createProgram()!;
    gl.attachShader(prog, sh(gl.VERTEX_SHADER, "attribute vec2 p;void main(){gl_Position=vec4(p,0.,1.);}"));
    gl.attachShader(prog, sh(gl.FRAGMENT_SHADER,
      "precision highp float;uniform float t;void main(){vec2 u=gl_FragCoord.xy/2048.;float a=t;" +
      "for(int i=0;i<600;i++){a=sin(a+u.x*3.1)*cos(a-u.y*2.7)+fract(a*1.37);}gl_FragColor=vec4(a,u,1.);}"));
    gl.linkProgram(prog);
    gl.useProgram(prog);
    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
    const loc = gl.getAttribLocation(prog, "p");
    gl.enableVertexAttribArray(loc);
    gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
    const tl = gl.getUniformLocation(prog, "t");
    gl.viewport(0, 0, 2048, 2048);
    const end = performance.now() + ms;
    const frame = () => {
      for (let i = 0; i < 4; i++) {
        gl.uniform1f(tl, performance.now() + i);
        gl.drawArrays(gl.TRIANGLES, 0, 3);
      }
      gl.finish();
      if (performance.now() < end) requestAnimationFrame(frame);
      else {
        gl.getExtension("WEBGL_lose_context")?.loseContext();
        resolve();
      }
    };
    requestAnimationFrame(frame);
  });
}
