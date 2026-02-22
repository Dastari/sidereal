import { useCallback, useEffect, useRef } from 'react'
import {
  edgeFragmentShader,
  edgeVertexShader,
  gridFragmentShader,
  gridVertexShader,
  nodeFragmentShader,
  nodeVertexShader,
} from './shaders'
import type { Camera, ExpandedNode, GraphEdge } from './types'

interface RendererState {
  gl: WebGL2RenderingContext
  gridProgram: WebGLProgram
  nodeProgram: WebGLProgram
  edgeProgram: WebGLProgram
  quadBuffer: WebGLBuffer
  nodeBuffer: WebGLBuffer
  edgeBuffer: WebGLBuffer
  gridLocs: {
    aPosition: number
    uResolution: WebGLUniformLocation | null
    uCamera: WebGLUniformLocation | null
    uZoom: WebGLUniformLocation | null
    uGridMajor: WebGLUniformLocation | null
    uGridMinor: WebGLUniformLocation | null
    uGridMicro: WebGLUniformLocation | null
    uBackground: WebGLUniformLocation | null
  }
  nodeLocs: {
    aPosition: number
    aColor: number
    aSize: number
    aSelected: number
    aHovered: number
    uResolution: WebGLUniformLocation | null
    uCamera: WebGLUniformLocation | null
    uZoom: WebGLUniformLocation | null
  }
  edgeLocs: {
    aPosition: number
    aColor: number
    uResolution: WebGLUniformLocation | null
    uCamera: WebGLUniformLocation | null
    uZoom: WebGLUniformLocation | null
  }
}

function createShader(
  gl: WebGL2RenderingContext,
  type: number,
  source: string,
): WebGLShader {
  const shader = gl.createShader(type)
  if (!shader) throw new Error('Failed to create shader')
  gl.shaderSource(shader, source)
  gl.compileShader(shader)
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const info = gl.getShaderInfoLog(shader)
    gl.deleteShader(shader)
    throw new Error(`Shader compile error: ${info}`)
  }
  return shader
}

function createProgram(
  gl: WebGL2RenderingContext,
  vs: string,
  fs: string,
): WebGLProgram {
  const program = gl.createProgram()
  // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition -- WebGL can return null
  if (!program) throw new Error('Failed to create program')
  gl.attachShader(program, createShader(gl, gl.VERTEX_SHADER, vs))
  gl.attachShader(program, createShader(gl, gl.FRAGMENT_SHADER, fs))
  gl.linkProgram(program)
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    const info = gl.getProgramInfoLog(program)
    gl.deleteProgram(program)
    throw new Error(`Program link error: ${info}`)
  }
  return program
}

const ENTITY_COLORS: Record<string, [number, number, number]> = {
  ship: [0.44, 0.75, 0.98],
  station: [0.7, 0.55, 0.95],
  asteroid: [0.75, 0.65, 0.45],
  planet: [0.45, 0.8, 0.55],
  component: [0.95, 0.7, 0.4],
  default: [0.6, 0.7, 0.85],
}

function getEntityColor(kind: string): [number, number, number] {
  const lower = kind.toLowerCase()
  const color = ENTITY_COLORS[lower]
  // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition -- runtime key lookup
  if (color) return color
  return [0.6, 0.7, 0.85]
}

export function useGridRenderer(
  canvasRef: React.RefObject<HTMLCanvasElement | null>,
  isDark: boolean,
) {
  const rendererRef = useRef<RendererState | null>(null)
  const rafRef = useRef<number>(0)

  const init = useCallback(() => {
    const canvas = canvasRef.current
    if (!canvas) return null

    const gl = canvas.getContext('webgl2', {
      antialias: true,
      alpha: false,
    })
    if (!gl) {
      console.error('WebGL2 not supported')
      return null
    }

    // Create programs
    const gridProgram = createProgram(gl, gridVertexShader, gridFragmentShader)
    const nodeProgram = createProgram(gl, nodeVertexShader, nodeFragmentShader)
    const edgeProgram = createProgram(gl, edgeVertexShader, edgeFragmentShader)

    // Create fullscreen quad for grid
    const quadBuffer = gl.createBuffer()
    gl.bindBuffer(gl.ARRAY_BUFFER, quadBuffer)
    gl.bufferData(
      gl.ARRAY_BUFFER,
      new Float32Array([0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 1, 1]),
      gl.STATIC_DRAW,
    )

    const nodeBuffer = gl.createBuffer()
    const edgeBuffer = gl.createBuffer()

    const state: RendererState = {
      gl,
      gridProgram,
      nodeProgram,
      edgeProgram,
      quadBuffer,
      nodeBuffer,
      edgeBuffer,
      gridLocs: {
        aPosition: gl.getAttribLocation(gridProgram, 'a_position'),
        uResolution: gl.getUniformLocation(gridProgram, 'u_resolution'),
        uCamera: gl.getUniformLocation(gridProgram, 'u_camera'),
        uZoom: gl.getUniformLocation(gridProgram, 'u_zoom'),
        uGridMajor: gl.getUniformLocation(gridProgram, 'u_gridMajor'),
        uGridMinor: gl.getUniformLocation(gridProgram, 'u_gridMinor'),
        uGridMicro: gl.getUniformLocation(gridProgram, 'u_gridMicro'),
        uBackground: gl.getUniformLocation(gridProgram, 'u_background'),
      },
      nodeLocs: {
        aPosition: gl.getAttribLocation(nodeProgram, 'a_position'),
        aColor: gl.getAttribLocation(nodeProgram, 'a_color'),
        aSize: gl.getAttribLocation(nodeProgram, 'a_size'),
        aSelected: gl.getAttribLocation(nodeProgram, 'a_selected'),
        aHovered: gl.getAttribLocation(nodeProgram, 'a_hovered'),
        uResolution: gl.getUniformLocation(nodeProgram, 'u_resolution'),
        uCamera: gl.getUniformLocation(nodeProgram, 'u_camera'),
        uZoom: gl.getUniformLocation(nodeProgram, 'u_zoom'),
      },
      edgeLocs: {
        aPosition: gl.getAttribLocation(edgeProgram, 'a_position'),
        aColor: gl.getAttribLocation(edgeProgram, 'a_color'),
        uResolution: gl.getUniformLocation(edgeProgram, 'u_resolution'),
        uCamera: gl.getUniformLocation(edgeProgram, 'u_camera'),
        uZoom: gl.getUniformLocation(edgeProgram, 'u_zoom'),
      },
    }

    rendererRef.current = state
    return state
  }, [canvasRef])

  const resize = useCallback(() => {
    const canvas = canvasRef.current
    const state = rendererRef.current
    if (!canvas || !state) return

    const dpr = window.devicePixelRatio || 1
    const width = canvas.clientWidth
    const height = canvas.clientHeight

    canvas.width = Math.floor(width * dpr)
    canvas.height = Math.floor(height * dpr)
    state.gl.viewport(0, 0, canvas.width, canvas.height)
  }, [canvasRef])

  const render = useCallback(
    (
      camera: Camera,
      nodes: Map<string, ExpandedNode>,
      edges: Array<GraphEdge>,
      selectedId: string | null,
      hoveredId: string | null,
    ) => {
      const canvas = canvasRef.current
      let state = rendererRef.current

      if (!canvas) return
      if (!state) {
        state = init()
        if (!state) return
      }

      const { gl } = state

      // Theme colors
      const bgColor: [number, number, number] = isDark
        ? [0.055, 0.07, 0.12]
        : [0.96, 0.97, 0.98]
      const gridMajor: [number, number, number] = isDark
        ? [0.25, 0.3, 0.4]
        : [0.7, 0.72, 0.75]
      const gridMinor: [number, number, number] = isDark
        ? [0.15, 0.18, 0.25]
        : [0.82, 0.84, 0.86]
      const gridMicro: [number, number, number] = isDark
        ? [0.1, 0.12, 0.17]
        : [0.9, 0.91, 0.92]

      gl.clearColor(...bgColor, 1)
      gl.clear(gl.COLOR_BUFFER_BIT)

      const width = canvas.width
      const height = canvas.height

      // Draw grid
      gl.useProgram(state.gridProgram)
      gl.bindBuffer(gl.ARRAY_BUFFER, state.quadBuffer)
      gl.enableVertexAttribArray(state.gridLocs.aPosition)
      gl.vertexAttribPointer(state.gridLocs.aPosition, 2, gl.FLOAT, false, 0, 0)
      gl.uniform2f(state.gridLocs.uResolution, width, height)
      gl.uniform2f(state.gridLocs.uCamera, camera.x, camera.y)
      gl.uniform1f(state.gridLocs.uZoom, camera.zoom)
      gl.uniform3fv(state.gridLocs.uGridMajor, gridMajor)
      gl.uniform3fv(state.gridLocs.uGridMinor, gridMinor)
      gl.uniform3fv(state.gridLocs.uGridMicro, gridMicro)
      gl.uniform3fv(state.gridLocs.uBackground, bgColor)
      gl.drawArrays(gl.TRIANGLES, 0, 6)

      // Filter edges to only those with both endpoints visible
      const visibleEdges = edges.filter(
        (e) => nodes.has(e.from) && nodes.has(e.to),
      )

      // Draw edges
      if (visibleEdges.length > 0) {
        const edgeData = new Float32Array(visibleEdges.length * 10)
        let ei = 0
        for (const edge of visibleEdges) {
          const fromNode = nodes.get(edge.from)
          const toNode = nodes.get(edge.to)
          if (!fromNode || !toNode) continue

          const color: [number, number, number] = isDark
            ? [0.35, 0.45, 0.6]
            : [0.5, 0.55, 0.65]

          edgeData[ei++] = fromNode.x
          edgeData[ei++] = fromNode.y
          edgeData[ei++] = color[0]
          edgeData[ei++] = color[1]
          edgeData[ei++] = color[2]
          edgeData[ei++] = toNode.x
          edgeData[ei++] = toNode.y
          edgeData[ei++] = color[0]
          edgeData[ei++] = color[1]
          edgeData[ei++] = color[2]
        }

        gl.useProgram(state.edgeProgram)
        gl.bindBuffer(gl.ARRAY_BUFFER, state.edgeBuffer)
        gl.bufferData(gl.ARRAY_BUFFER, edgeData, gl.DYNAMIC_DRAW)

        const stride = 20 // 5 floats * 4 bytes
        gl.enableVertexAttribArray(state.edgeLocs.aPosition)
        gl.vertexAttribPointer(
          state.edgeLocs.aPosition,
          2,
          gl.FLOAT,
          false,
          stride,
          0,
        )
        gl.enableVertexAttribArray(state.edgeLocs.aColor)
        gl.vertexAttribPointer(
          state.edgeLocs.aColor,
          3,
          gl.FLOAT,
          false,
          stride,
          8,
        )
        gl.uniform2f(state.edgeLocs.uResolution, width, height)
        gl.uniform2f(state.edgeLocs.uCamera, camera.x, camera.y)
        gl.uniform1f(state.edgeLocs.uZoom, camera.zoom)

        gl.enable(gl.BLEND)
        gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA)
        gl.drawArrays(gl.LINES, 0, visibleEdges.length * 2)
      }

      // Draw nodes
      if (nodes.size > 0) {
        const nodeData = new Float32Array(nodes.size * 8)
        let ni = 0

        for (const [id, node] of nodes) {
          const color = getEntityColor(node.kind)
          const baseSize = node.depth === 0 ? 20 : 12

          nodeData[ni++] = node.x
          nodeData[ni++] = node.y
          nodeData[ni++] = color[0]
          nodeData[ni++] = color[1]
          nodeData[ni++] = color[2]
          nodeData[ni++] = baseSize
          nodeData[ni++] = id === selectedId ? 1 : 0
          nodeData[ni++] = id === hoveredId ? 1 : 0
        }

        gl.useProgram(state.nodeProgram)
        gl.bindBuffer(gl.ARRAY_BUFFER, state.nodeBuffer)
        gl.bufferData(gl.ARRAY_BUFFER, nodeData, gl.DYNAMIC_DRAW)

        const stride = 32 // 8 floats * 4 bytes
        gl.enableVertexAttribArray(state.nodeLocs.aPosition)
        gl.vertexAttribPointer(
          state.nodeLocs.aPosition,
          2,
          gl.FLOAT,
          false,
          stride,
          0,
        )
        gl.enableVertexAttribArray(state.nodeLocs.aColor)
        gl.vertexAttribPointer(
          state.nodeLocs.aColor,
          3,
          gl.FLOAT,
          false,
          stride,
          8,
        )
        gl.enableVertexAttribArray(state.nodeLocs.aSize)
        gl.vertexAttribPointer(
          state.nodeLocs.aSize,
          1,
          gl.FLOAT,
          false,
          stride,
          20,
        )
        gl.enableVertexAttribArray(state.nodeLocs.aSelected)
        gl.vertexAttribPointer(
          state.nodeLocs.aSelected,
          1,
          gl.FLOAT,
          false,
          stride,
          24,
        )
        gl.enableVertexAttribArray(state.nodeLocs.aHovered)
        gl.vertexAttribPointer(
          state.nodeLocs.aHovered,
          1,
          gl.FLOAT,
          false,
          stride,
          28,
        )

        gl.uniform2f(state.nodeLocs.uResolution, width, height)
        gl.uniform2f(state.nodeLocs.uCamera, camera.x, camera.y)
        gl.uniform1f(state.nodeLocs.uZoom, camera.zoom)

        gl.enable(gl.BLEND)
        gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA)
        gl.drawArrays(gl.POINTS, 0, nodes.size)
      }
    },
    [canvasRef, init, isDark],
  )

  useEffect(() => {
    return () => {
      if (rafRef.current) {
        cancelAnimationFrame(rafRef.current)
      }
    }
  }, [])

  return { init, resize, render }
}
