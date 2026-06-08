import { useSceneStore } from '../store/scene'

/// The viewport shows the latest GPU-rendered frame. The app auto-previews
/// after edits and on scrub; the button forces an immediate re-render.
export function Viewport() {
  const previewUrl = useSceneStore((s) => s.previewUrl)
  const renderPreview = useSceneStore((s) => s.renderPreview)
  const scene = useSceneStore((s) => s.scene)
  const frame = useSceneStore((s) => s.frame)

  return (
    <div style={s.container}>
      <div style={s.bar}>
        <button style={s.previewBtn} onClick={() => renderPreview(frame)}>Render Frame {frame}</button>
        <span style={s.info}>{scene?.elements.length ?? 0} elements · {scene?.render.width}×{scene?.render.height}</span>
      </div>
      <div style={s.stage}>
        {previewUrl ? (
          <img src={previewUrl} style={s.img} alt={`frame ${frame}`} />
        ) : (
          <div style={s.empty}>
            <div style={s.emptyIcon}>⚡</div>
            <div style={s.emptyTitle}>Add elements, then Preview</div>
            <div style={s.emptyHint}>Native wgpu renderer · Lite mode</div>
          </div>
        )}
      </div>
    </div>
  )
}

const s: Record<string, React.CSSProperties> = {
  container: { display: 'flex', flexDirection: 'column', height: '100%', background: '#0a0a10' },
  bar: { display: 'flex', alignItems: 'center', gap: 10, padding: '6px 12px', background: '#0f0f16', borderBottom: '1px solid #1a1a22' },
  previewBtn: { padding: '4px 12px', background: '#1e1e2a', border: '1px solid #2a2a3a', borderRadius: 5, color: '#9988cc', fontSize: 11, cursor: 'pointer', fontWeight: 600 },
  info: { fontSize: 11, color: '#333348' },
  stage: { flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', overflow: 'hidden', background: '#08080c' },
  img: { maxWidth: '100%', maxHeight: '100%', objectFit: 'contain', display: 'block' },
  empty: { display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 10 },
  emptyIcon: { fontSize: 48 },
  emptyTitle: { fontSize: 15, color: '#333348', fontWeight: 600 },
  emptyHint: { fontSize: 12, color: '#222232' },
}
