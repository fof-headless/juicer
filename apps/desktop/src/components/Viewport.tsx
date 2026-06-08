import { useCallback, useEffect, useRef, useState } from 'react'
import { useSceneStore } from '../store/scene'
import type { SceneData, Element } from '../store/scene'

// ── Math helpers ──────────────────────────────────────────────────────────────

type V3 = [number, number, number]
type M4 = number[] // 16-element row-major

function dot(a: V3, b: V3) { return a[0]*b[0]+a[1]*b[1]+a[2]*b[2] }
function sub(a: V3, b: V3): V3 { return [a[0]-b[0],a[1]-b[1],a[2]-b[2]] }
function add(a: V3, b: V3): V3 { return [a[0]+b[0],a[1]+b[1],a[2]+b[2]] }
function scale(v: V3, s: number): V3 { return [v[0]*s, v[1]*s, v[2]*s] }
function len(v: V3) { return Math.sqrt(dot(v,v)) }
function norm(v: V3): V3 { const l = len(v)||1; return [v[0]/l,v[1]/l,v[2]/l] }
function cross(a: V3, b: V3): V3 { return [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]] }

function m4identity(): M4 { return [1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1] }

function m4mul(a: M4, b: M4): M4 {
  const c: M4 = new Array(16).fill(0)
  for (let r=0;r<4;r++) for (let col=0;col<4;col++)
    for (let k=0;k<4;k++) c[r*4+col]+=a[r*4+k]*b[k*4+col]
  return c
}

function lookAt(eye: V3, target: V3, up: V3 = [0,1,0]): M4 {
  const f = norm(sub(target, eye))
  const r = norm(cross(f, up))
  const u = cross(r, f)
  return [
    r[0],  r[1],  r[2],  -dot(r,eye),
    u[0],  u[1],  u[2],  -dot(u,eye),
   -f[0], -f[1], -f[2],   dot(f,eye),
    0,     0,     0,      1,
  ]
}

function perspective(fovDeg: number, aspect: number, near: number, far: number): M4 {
  const f = 1/Math.tan(fovDeg*Math.PI/360)
  const nf = 1/(near-far)
  return [
    f/aspect, 0,  0,               0,
    0,        f,  0,               0,
    0,        0,  (far+near)*nf,   2*far*near*nf,
    0,        0, -1,               0,
  ]
}

function m4TransformV3(m: M4, v: V3, w=1): [number,number,number,number] {
  const x=m[0]*v[0]+m[1]*v[1]+m[2]*v[2]+m[3]*w
  const y=m[4]*v[0]+m[5]*v[1]+m[6]*v[2]+m[7]*w
  const z=m[8]*v[0]+m[9]*v[1]+m[10]*v[2]+m[11]*w
  const q=m[12]*v[0]+m[13]*v[1]+m[14]*v[2]+m[15]*w
  return [x,y,z,q]
}

/** Project a world point → canvas [px, py]. Returns null if behind camera. */
function project(worldPt: V3, vp: M4, cw: number, ch: number): [number,number]|null {
  const [cx,cy,,cw2] = m4TransformV3(vp, worldPt)
  if (cw2 <= 0) return null
  const nx = cx/cw2, ny = cy/cw2
  return [(nx+1)*0.5*cw, (1-ny)*0.5*ch]
}

// ── Element transform helpers ─────────────────────────────────────────────────

function makeRotX(rad: number): M4 {
  const c=Math.cos(rad), s=Math.sin(rad)
  return [1,0,0,0, 0,c,-s,0, 0,s,c,0, 0,0,0,1]
}
function makeRotY(rad: number): M4 {
  const c=Math.cos(rad), s=Math.sin(rad)
  return [c,0,s,0, 0,1,0,0, -s,0,c,0, 0,0,0,1]
}
function makeRotZ(rad: number): M4 {
  const c=Math.cos(rad), s=Math.sin(rad)
  return [c,-s,0,0, s,c,0,0, 0,0,1,0, 0,0,0,1]
}

function elementTransform(el: Element): M4 {
  const [px,py,pz] = el.position
  const [rx,ry,rz] = el.rotation
  const [sx,sy,sz] = el.scale
  const sw = el.kind === 'plane' ? el.width : 1
  const sh = el.kind === 'plane' ? el.height : 1

  const T: M4 = [1,0,0,px, 0,1,0,py, 0,0,1,pz, 0,0,0,1]
  const R = m4mul(m4mul(makeRotX(rx), makeRotY(ry)), makeRotZ(rz))
  const S: M4 = [sx*sw,0,0,0, 0,sy*sh,0,0, 0,0,sz,0, 0,0,0,1]
  return m4mul(T, m4mul(R, S))
}

/** 4 local corners of a unit plane (-0.5..0.5 xy) */
const PLANE_CORNERS: V3[] = [[-0.5,-0.5,0],[0.5,-0.5,0],[0.5,0.5,0],[-0.5,0.5,0]]

/** 8 corners of a unit cube */
const CUBE_CORNERS: V3[] = [
  [-0.5,-0.5,-0.5],[0.5,-0.5,-0.5],[0.5,0.5,-0.5],[-0.5,0.5,-0.5],
  [-0.5,-0.5, 0.5],[0.5,-0.5, 0.5],[0.5,0.5, 0.5],[-0.5,0.5, 0.5],
]

function transformPt(m: M4, v: V3): V3 {
  const [x,y,z] = m4TransformV3(m, v)
  return [x,y,z]
}

function hexToRgba(hex: string, alpha: number): string {
  const h = hex.replace('#','')
  const r=parseInt(h.slice(0,2),16), g=parseInt(h.slice(2,4),16), b=parseInt(h.slice(4,6),16)
  return `rgba(${r},${g},${b},${alpha})`
}

// ── Point-in-polygon hit test ─────────────────────────────────────────────────
function pointInPoly(px: number, py: number, poly: [number,number][]): boolean {
  let inside = false
  const n = poly.length
  for (let i=0, j=n-1; i<n; j=i++) {
    const [xi,yi]=poly[i],[xj,yj]=poly[j]
    if ((yi>py)!==(yj>py) && px<(xj-xi)*(py-yi)/(yj-yi)+xi) inside=!inside
  }
  return inside
}

// ── Draw scene on canvas ──────────────────────────────────────────────────────

function drawScene(
  canvas: HTMLCanvasElement,
  scene: SceneData,
  selectedId: string|null,
  textures: Map<string,HTMLImageElement>
) {
  const ctx = canvas.getContext('2d')!
  const cw = canvas.width, ch = canvas.height

  // Clear
  const bg = scene.render.background
  ctx.fillStyle = `rgba(${(bg[0]*255)|0},${(bg[1]*255)|0},${(bg[2]*255)|0},1)`
  ctx.fillRect(0, 0, cw, ch)

  const view = lookAt(scene.camera.position, scene.camera.target)
  const proj = perspective(scene.camera.fov_deg, cw/ch, scene.camera.near, scene.camera.far)
  const vp = m4mul(proj, view)

  // Sort back-to-front by distance to camera for correct alpha blending
  const camPos = scene.camera.position
  const sorted = [...scene.elements]
    .filter(e => e.visible && e.opacity > 0)
    .sort((a,b) => {
      const da = len(sub(a.position, camPos))
      const db = len(sub(b.position, camPos))
      return db - da
    })

  for (const el of sorted) {
    const model = elementTransform(el)
    const isSel = el.id === selectedId
    const alpha = el.opacity

    if (el.kind === 'plane') {
      const pts = PLANE_CORNERS.map(c => {
        const w = transformPt(model, c)
        return project(w, vp, cw, ch)
      })
      if (pts.some(p => p===null)) continue
      const poly = pts as [number,number][]

      ctx.save()
      ctx.beginPath()
      ctx.moveTo(poly[0][0], poly[0][1])
      for (let i=1;i<4;i++) ctx.lineTo(poly[i][0], poly[i][1])
      ctx.closePath()

      // Try to draw texture if loaded
      const tex = el.image_path ? textures.get(el.image_path) : null
      if (tex) {
        // Map image onto the projected quad via canvas transform trick
        // We use the first three points to compute an affine transform
        const [p0,p1,p3] = [poly[0],poly[1],poly[3]]
        const [p2] = [poly[2]]
        // Use drawImage with a clipping path and simple transform (approximate)
        ctx.globalAlpha = alpha
        ctx.clip()
        // Affine transform from unit square to screen quad (approximate for small perspective)
        const dx1=p1[0]-p0[0], dy1=p1[1]-p0[1]
        const dx2=p3[0]-p0[0], dy2=p3[1]-p0[1]
        ctx.transform(dx1, dy1, dx2, dy2, p0[0], p0[1])
        ctx.drawImage(tex, 0, 0, 1, 1)
      } else {
        ctx.globalAlpha = alpha * 0.75
        ctx.fillStyle = hexToRgba(el.color, 1)
        ctx.fill()
        ctx.globalAlpha = alpha
      }

      // Outline
      ctx.globalAlpha = 1
      ctx.strokeStyle = isSel ? '#6644ff' : 'rgba(255,255,255,0.15)'
      ctx.lineWidth = isSel ? 2 : 1
      ctx.stroke()
      ctx.restore()

      // Label
      const cx = poly.reduce((s,p)=>s+p[0],0)/4
      const cy = poly.reduce((s,p)=>s+p[1],0)/4
      ctx.save()
      ctx.globalAlpha = 0.85
      ctx.font = `${isSel?'600 ':''}11px Inter, system-ui, sans-serif`
      ctx.fillStyle = isSel ? '#aa88ff' : '#aaaacc'
      ctx.textAlign = 'center'
      ctx.textBaseline = 'middle'
      ctx.fillText(el.name, cx, cy)
      ctx.restore()

    } else if (el.kind === 'box') {
      const corners = CUBE_CORNERS.map(c => {
        const w = transformPt(model, c)
        return project(w, vp, cw, ch)
      })
      // Draw edges
      const edges = [[0,1],[1,2],[2,3],[3,0],[4,5],[5,6],[6,7],[7,4],[0,4],[1,5],[2,6],[3,7]]
      ctx.save()
      ctx.strokeStyle = isSel ? '#6644ff' : hexToRgba(el.color, alpha)
      ctx.lineWidth = isSel ? 2 : 1.5
      ctx.globalAlpha = alpha
      for (const [a,b] of edges) {
        const pa=corners[a], pb=corners[b]
        if (!pa||!pb) continue
        ctx.beginPath(); ctx.moveTo(pa[0],pa[1]); ctx.lineTo(pb[0],pb[1]); ctx.stroke()
      }
      ctx.restore()

    } else if (el.kind === 'sphere') {
      const center = project(el.position, vp, cw, ch)
      if (!center) continue
      // Approximate radius by projecting a point offset by scale
      const [sx] = el.scale
      const edge = project(add(el.position,[sx,0,0]), vp, cw, ch)
      const r = edge ? Math.abs(edge[0]-center[0]) : 20
      ctx.save()
      ctx.globalAlpha = alpha
      ctx.beginPath()
      ctx.arc(center[0], center[1], Math.max(r,2), 0, Math.PI*2)
      ctx.fillStyle = hexToRgba(el.color, 0.65)
      ctx.fill()
      ctx.strokeStyle = isSel ? '#6644ff' : hexToRgba(el.color, 1)
      ctx.lineWidth = isSel ? 2 : 1
      ctx.stroke()
      ctx.restore()
    }
  }

  // Selection handles for selected plane
  if (selectedId) {
    const el = scene.elements.find(e => e.id === selectedId)
    if (el && el.kind === 'plane') {
      const model = elementTransform(el)
      const pts = PLANE_CORNERS.map(c => {
        const w = transformPt(model, c)
        return project(w, vp, cw, ch)
      })
      if (pts.every(p => p!==null)) {
        const poly = pts as [number,number][]
        ctx.save()
        ctx.fillStyle = '#6644ff'
        for (const p of poly) {
          ctx.beginPath(); ctx.arc(p[0],p[1],4,0,Math.PI*2); ctx.fill()
        }
        ctx.restore()
      }
    }
  }
}

// ── Component ─────────────────────────────────────────────────────────────────

export function Viewport() {
  const scene = useSceneStore(s => s.scene)
  const selectedId = useSceneStore(s => s.selectedId)
  const select = useSceneStore(s => s.select)
  const updateElement = useSceneStore(s => s.updateElement)
  const addElement = useSceneStore(s => s.addElement)
  const frame = useSceneStore(s => s.frame)
  const renderPreview = useSceneStore(s => s.renderPreview)
  const isRendering = useSceneStore(s => s.isRendering)
  const previewUrl = useSceneStore(s => s.previewUrl)

  const canvasRef = useRef<HTMLCanvasElement>(null)
  const containerRef = useRef<HTMLDivElement>(null)
  const texturesRef = useRef<Map<string,HTMLImageElement>>(new Map())
  const [showRender, setShowRender] = useState(false)

  // Load/cache image textures when scene changes
  useEffect(() => {
    if (!scene) return
    for (const el of scene.elements) {
      if (el.image_path && !texturesRef.current.has(el.image_path)) {
        const img = new Image()
        img.onload = () => {
          texturesRef.current.set(el.image_path!, img)
          redraw()
        }
        // Use Tauri asset URL if available
        const src = (window as any).__TAURI__?.core?.convertFileSrc
          ? (window as any).__TAURI__.core.convertFileSrc(el.image_path)
          : `file://${el.image_path}`
        img.src = src
        texturesRef.current.set(el.image_path, img) // placeholder until loaded
      }
    }
  }, [scene])

  const redraw = useCallback(() => {
    const canvas = canvasRef.current
    if (!canvas || !scene) return
    const container = containerRef.current
    if (container) {
      const { width, height } = container.getBoundingClientRect()
      if (canvas.width !== width || canvas.height !== height) {
        canvas.width = width; canvas.height = height
      }
    }
    drawScene(canvas, scene, selectedId, texturesRef.current)
  }, [scene, selectedId])

  useEffect(() => { redraw() }, [redraw, frame])

  // Resize observer
  useEffect(() => {
    const el = containerRef.current
    if (!el) return
    const ro = new ResizeObserver(() => redraw())
    ro.observe(el)
    return () => ro.disconnect()
  }, [redraw])

  // ── Interaction: click to select, drag to move ──────────────────────────────
  const dragRef = useRef<{
    id: string
    startMouse: [number,number]
    startPos: V3
    hitOffset: [number, number]
  } | null>(null)

  const getHitElement = useCallback((ex: number, ey: number): string|null => {
    if (!scene) return null
    const canvas = canvasRef.current!
    const cw = canvas.width, ch = canvas.height
    const view = lookAt(scene.camera.position, scene.camera.target)
    const proj = perspective(scene.camera.fov_deg, cw/ch, scene.camera.near, scene.camera.far)
    const vp = m4mul(proj, view)

    // Test in reverse z-order (front to back)
    const camPos = scene.camera.position
    const sorted = [...scene.elements]
      .filter(e => e.visible)
      .sort((a,b) => len(sub(a.position,camPos)) - len(sub(b.position,camPos)))

    for (const el of sorted) {
      const model = elementTransform(el)
      if (el.kind === 'plane') {
        const pts = PLANE_CORNERS.map(c => {
          const w = transformPt(model, c)
          return project(w, vp, cw, ch)
        })
        if (pts.every(p=>p!==null) && pointInPoly(ex,ey,pts as [number,number][])) return el.id
      } else if (el.kind === 'sphere') {
        const center = project(el.position, vp, cw, ch)
        const edge = project(add(el.position,[el.scale[0],0,0]), vp, cw, ch)
        if (center && edge) {
          const r = Math.abs(edge[0]-center[0])
          const dx=ex-center[0],dy=ey-center[1]
          if (dx*dx+dy*dy<=r*r) return el.id
        }
      } else {
        // Box: click near center
        const center = project(el.position, vp, cw, ch)
        if (center && Math.abs(ex-center[0])<40 && Math.abs(ey-center[1])<40) return el.id
      }
    }
    return null
  }, [scene])

  const onPointerDown = useCallback((e: React.PointerEvent<HTMLCanvasElement>) => {
    const rect = canvasRef.current!.getBoundingClientRect()
    const ex = e.clientX - rect.left, ey = e.clientY - rect.top
    const hitId = getHitElement(ex, ey)
    if (hitId) {
      select(hitId)
      const el = scene!.elements.find(e => e.id === hitId)!
      dragRef.current = { id: hitId, startMouse: [ex, ey], startPos: [...el.position] as V3, hitOffset: [0,0] }
      ;(e.target as HTMLElement).setPointerCapture(e.pointerId)
    } else {
      select(null)
    }
  }, [getHitElement, select, scene])

  const onPointerMove = useCallback((e: React.PointerEvent<HTMLCanvasElement>) => {
    if (e.buttons !== 1 || !dragRef.current || !scene) return
    const rect = canvasRef.current!.getBoundingClientRect()
    const ex = e.clientX - rect.left, ey = e.clientY - rect.top
    const [sx, sy] = dragRef.current.startMouse

    // Convert screen delta to world delta at the element's depth
    const canvas = canvasRef.current!
    const cw = canvas.width, ch = canvas.height
    const el = scene.elements.find(e => e.id === dragRef.current!.id)
    if (!el) return

    // Estimate world-space move: screen pixel → world unit at element's z distance
    const view = lookAt(scene.camera.position, scene.camera.target)
    const proj = perspective(scene.camera.fov_deg, cw/ch, scene.camera.near, scene.camera.far)
    const vp = m4mul(proj, view)
    const screenCenter = project(el.position, vp, cw, ch)
    if (!screenCenter) return

    // How many world units per pixel at this depth
    const worldPt1 = unproject([screenCenter[0]+1, screenCenter[1]], vp, el.position, cw, ch)
    const worldPt2 = unproject([screenCenter[0], screenCenter[1]+1], vp, el.position, cw, ch)
    if (!worldPt1||!worldPt2) return
    const unitsPerPixelX = Math.abs(worldPt1[0] - el.position[0])
    const unitsPerPixelY = Math.abs(worldPt2[1] - el.position[1])

    const dx = (ex - sx) * unitsPerPixelX
    const dy = -(ey - sy) * unitsPerPixelY  // screen Y is inverted

    const [opx,opy,opz] = dragRef.current.startPos
    updateElement(el.id, { position: [opx+dx, opy+dy, opz] })
  }, [scene, updateElement])

  const onPointerUp = useCallback(() => { dragRef.current = null }, [])

  const isEmpty = !scene || scene.elements.length === 0

  return (
    <div ref={containerRef} style={s.container}>
      <div style={s.bar}>
        <span style={s.barTitle}>Viewport</span>
        {scene && <span style={s.info}>{scene.elements.length} elements · frame {frame}</span>}
        <div style={{ marginLeft: 'auto', display: 'flex', gap: 6 }}>
          {scene && scene.elements.length > 0 && (
            <button
              style={{ ...s.barBtn, ...(showRender ? s.barBtnActive : {}) }}
              onClick={() => { setShowRender(v => !v); if (!showRender) renderPreview(frame) }}
            >
              {isRendering ? '…' : '⚡'} Render view
            </button>
          )}
        </div>
      </div>

      <div style={s.stage}>
        {isEmpty ? (
          <div style={s.empty}>
            <div style={s.emptyLogo}>⚡</div>
            <div style={s.emptyHeading}>Start building</div>
            <div style={s.steps}>
              <Step n={1} title="Add a shape" body="Click below to add your first element, or use the Scene panel on the left." />
              <Step n={2} title="Click to select, drag to move" body="Once added, click any object to select it, then drag to reposition." />
              <Step n={3} title="Keyframe → Render MP4" body="Scrub the timeline, insert keyframes in the Properties panel, then hit Render MP4." />
            </div>
            <div style={s.quickAdd}>
              <span style={s.quickLabel}>Quick add:</span>
              <button style={s.qBtn} onClick={() => addElement('plane','Plane 1',{color:'#2a1a6e',width:4,height:2.5})}>Plane</button>
              <button style={s.qBtn} onClick={() => addElement('box','Box 1',{color:'#4488ff'})}>Box</button>
              <button style={s.qBtn} onClick={() => addElement('sphere','Sphere 1',{color:'#ff4488'})}>Sphere</button>
            </div>
          </div>
        ) : (
          <>
            <canvas
              ref={canvasRef}
              style={s.canvas}
              onPointerDown={onPointerDown}
              onPointerMove={onPointerMove}
              onPointerUp={onPointerUp}
            />
            {showRender && previewUrl && (
              <img src={previewUrl} style={s.renderOverlay} alt="render" />
            )}
          </>
        )}
      </div>

      {!isEmpty && (
        <div style={s.hint}>Click to select · Drag to move · Shift-drag coming soon</div>
      )}
    </div>
  )
}

/** Estimate world-space point on the plane z=ref[2] from a screen coordinate. */
function unproject(screen: [number,number], vp: M4, ref: V3, cw: number, ch: number): V3|null {
  // Project ref point to get its NDC, compute delta NDC, scale by ref's clip-w
  const [rx,ry,,rw] = m4TransformV3(vp, ref)
  if (rw<=0) return null
  const nRefX = rx/rw, nRefY = ry/rw
  const nx = (screen[0]/cw)*2-1
  const ny = 1-(screen[1]/ch)*2
  const dndcX = nx - nRefX, dndcY = ny - nRefY
  // Approximate inverse: scale by rw/proj[0,0] and rw/proj[1,1]
  // For small drag deltas this is accurate enough for UX
  const worldDX = dndcX * rw  // in view space units
  const worldDY = dndcY * rw
  return [ref[0]+worldDX, ref[1]+worldDY, ref[2]]
}

function Step({ n, title, body }: { n: number; title: string; body: string }) {
  return (
    <div style={st.step}>
      <div style={st.num}>{n}</div>
      <div>
        <div style={st.title}>{title}</div>
        <div style={st.body}>{body}</div>
      </div>
    </div>
  )
}

const st: Record<string, React.CSSProperties> = {
  step: { display: 'flex', gap: 14, alignItems: 'flex-start', textAlign: 'left', maxWidth: 360 },
  num: { width: 24, height: 24, borderRadius: '50%', background: '#5533bb', color: 'white', fontWeight: 700, fontSize: 12, display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0, marginTop: 1 },
  title: { color: '#b0b0cc', fontSize: 13, fontWeight: 600, marginBottom: 3 },
  body: { color: '#44445a', fontSize: 11, lineHeight: 1.6 },
}

const s: Record<string, React.CSSProperties> = {
  container: { display: 'flex', flexDirection: 'column', height: '100%', background: '#0a0a10' },
  bar: { display: 'flex', alignItems: 'center', gap: 10, padding: '5px 12px', background: '#0f0f16', borderBottom: '1px solid #1a1a22', flexShrink: 0 },
  barTitle: { fontSize: 10, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.09em', color: '#555570' },
  info: { fontSize: 11, color: '#333348' },
  barBtn: { padding: '3px 9px', background: '#1e1e2a', border: '1px solid #2a2a3a', borderRadius: 5, color: '#9988cc', fontSize: 11, cursor: 'pointer', fontWeight: 600 },
  barBtnActive: { background: '#33225a', borderColor: '#6644ff', color: '#bb99ff' },
  stage: { flex: 1, position: 'relative', overflow: 'hidden' },
  canvas: { position: 'absolute', inset: 0, width: '100%', height: '100%', cursor: 'default', display: 'block' },
  renderOverlay: { position: 'absolute', inset: 0, width: '100%', height: '100%', objectFit: 'contain', opacity: 0.85, pointerEvents: 'none' },
  hint: { padding: '3px 12px', fontSize: 10, color: '#2a2a3a', background: '#08080c', flexShrink: 0 },
  empty: { position: 'absolute', inset: 0, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 20, padding: '0 24px' },
  emptyLogo: { fontSize: 40 },
  emptyHeading: { fontSize: 20, color: '#6644ff', fontWeight: 700, letterSpacing: '-0.02em' },
  steps: { display: 'flex', flexDirection: 'column', gap: 16, width: '100%', maxWidth: 400 },
  quickAdd: { display: 'flex', alignItems: 'center', gap: 8 },
  quickLabel: { fontSize: 11, color: '#44445a' },
  qBtn: { padding: '5px 14px', background: '#1e1e2a', border: '1px solid #2a2a3a', borderRadius: 5, color: '#9988cc', fontSize: 11, cursor: 'pointer', fontWeight: 600 },
}
