import { useSceneStore } from '../store/scene'

export function Viewport() {
  const previewUrl = useSceneStore((s) => s.previewUrl)
  const renderPreview = useSceneStore((s) => s.renderPreview)
  const scene = useSceneStore((s) => s.scene)
  const frame = useSceneStore((s) => s.frame)
  const addElement = useSceneStore((s) => s.addElement)

  const isEmpty = !scene || scene.elements.length === 0

  return (
    <div style={s.container}>
      <div style={s.bar}>
        <button style={s.previewBtn} onClick={() => renderPreview(frame)}>▶ Render Frame {frame}</button>
        <span style={s.info}>{scene?.elements.length ?? 0} elements · {scene?.render.width}×{scene?.render.height} · frame {frame}</span>
      </div>
      <div style={s.stage}>
        {previewUrl && !isEmpty ? (
          <img src={previewUrl} style={s.img} alt={`frame ${frame}`} />
        ) : (
          <div style={s.empty}>
            <div style={s.emptyLogo}>⚡</div>
            <div style={s.emptyHeading}>Welcome to Juicer</div>
            <div style={s.emptySubtitle}>Build your first scene in 3 steps</div>
            <div style={s.steps}>
              <Step n={1} title="Add an element" body="In the left panel, click Plane / Box / Sphere — or paste HTML below and click Capture & Add." />
              <Step n={2} title="Keyframe it" body="Select the element → drag to a frame in the timeline below → open Properties (right panel) → Insert Keyframe." />
              <Step n={3} title="Preview & Render" body='Click "▶ Render Frame" above to see the wgpu output. Hit Render MP4 in the toolbar when done.' />
            </div>
            <div style={s.quickAdd}>
              <span style={s.quickLabel}>Quick add:</span>
              <button style={s.qBtn} onClick={() => addElement('plane', 'Plane 1', { color: '#2a1a6e', width: 4, height: 2.5 })}>Plane</button>
              <button style={s.qBtn} onClick={() => addElement('box', 'Box 1', { color: '#4488ff' })}>Box</button>
              <button style={s.qBtn} onClick={() => addElement('sphere', 'Sphere 1', { color: '#ff4488' })}>Sphere</button>
            </div>
          </div>
        )}
      </div>
    </div>
  )
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
  bar: { display: 'flex', alignItems: 'center', gap: 10, padding: '6px 12px', background: '#0f0f16', borderBottom: '1px solid #1a1a22' },
  previewBtn: { padding: '4px 12px', background: '#1e1e2a', border: '1px solid #2a2a3a', borderRadius: 5, color: '#9988cc', fontSize: 11, cursor: 'pointer', fontWeight: 600 },
  info: { fontSize: 11, color: '#333348' },
  stage: { flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', overflow: 'hidden', background: '#08080c' },
  img: { maxWidth: '100%', maxHeight: '100%', objectFit: 'contain', display: 'block' },
  empty: { display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 20, padding: '0 24px', maxWidth: 480 },
  emptyLogo: { fontSize: 40 },
  emptyHeading: { fontSize: 22, color: '#6644ff', fontWeight: 700, letterSpacing: '-0.02em' },
  emptySubtitle: { fontSize: 13, color: '#44445a', marginTop: -10 },
  steps: { display: 'flex', flexDirection: 'column', gap: 18, width: '100%' },
  quickAdd: { display: 'flex', alignItems: 'center', gap: 8, marginTop: 4 },
  quickLabel: { fontSize: 11, color: '#44445a' },
  qBtn: { padding: '5px 14px', background: '#1e1e2a', border: '1px solid #2a2a3a', borderRadius: 5, color: '#9988cc', fontSize: 11, cursor: 'pointer', fontWeight: 600 },
}
