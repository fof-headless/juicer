import { useState } from 'react'
import { useSceneStore } from '../store/scene'

export function Properties() {
  const scene = useSceneStore((s) => s.scene)
  const selectedId = useSceneStore((s) => s.selectedId)
  const frame = useSceneStore((s) => s.frame)
  const updateElement = useSceneStore((s) => s.updateElement)
  const setKeyframe = useSceneStore((s) => s.setKeyframe)

  const [kfProp, setKfProp] = useState<'position' | 'rotation' | 'scale' | 'opacity'>('position')
  const [easing, setEasing] = useState('ease-in-out')

  const el = scene?.elements.find((e) => e.id === selectedId)

  if (!el) {
    return (
      <div style={{ ...s.panel, alignItems: 'center', justifyContent: 'center', gap: 8, padding: 20 }}>
        <div style={{ fontSize: 22, opacity: 0.3 }}>☰</div>
        <div style={{ color: '#444460', textAlign: 'center', fontSize: 11, lineHeight: 1.7 }}>
          Click an element in the<br />Scene list to edit its<br />properties and keyframes.
        </div>
      </div>
    )
  }

  const insertKeyframe = () => {
    let value: unknown
    if (kfProp === 'position') value = el.position
    else if (kfProp === 'rotation') value = el.rotation
    else if (kfProp === 'scale') value = el.scale
    else value = el.opacity
    setKeyframe(el.id, frame, kfProp, value, easing)
  }

  return (
    <div style={s.panel}>
      <div style={s.header}>Properties — {el.name}</div>

      <Section title="Name">
        <input style={s.input} value={el.name} onChange={(e) => updateElement(el.id, { name: e.target.value })} />
      </Section>

      <Section title="Transform">
        <Vec3 label="Loc" value={el.position} onChange={(v) => updateElement(el.id, { position: v })} step={0.1} />
        <Vec3 label="Rot" value={el.rotation} onChange={(v) => updateElement(el.id, { rotation: v })} step={0.05} />
        <Vec3 label="Scl" value={el.scale} onChange={(v) => updateElement(el.id, { scale: v })} step={0.1} />
      </Section>

      <Section title="Appearance">
        <Row label="Color">
          <input type="color" style={s.color} value={el.color} onChange={(e) => updateElement(el.id, { color: e.target.value })} />
        </Row>
        <Row label="Opacity">
          <input type="range" min={0} max={1} step={0.01} value={el.opacity} onChange={(e) => updateElement(el.id, { opacity: parseFloat(e.target.value) })} style={{ flex: 1, accentColor: '#6644ff' }} />
          <span style={s.val}>{(el.opacity * 100).toFixed(0)}%</span>
        </Row>
        {el.kind === 'plane' && (
          <>
            <Row label="Width"><Num value={el.width} onChange={(v) => updateElement(el.id, { width: v })} /></Row>
            <Row label="Height"><Num value={el.height} onChange={(v) => updateElement(el.id, { height: v })} /></Row>
            <Row label="Unlit">
              <input type="checkbox" checked={el.unlit} onChange={(e) => updateElement(el.id, { unlit: e.target.checked })} />
            </Row>
          </>
        )}
      </Section>

      <Section title="Keyframe at Frame">
        <Row label="Frame"><span style={{ ...s.val, color: '#6644ff', fontWeight: 700 }}>f{frame}</span><span style={{ fontSize: 10, color: '#333344', marginLeft: 4 }}>← drag timeline below</span></Row>
        <Row label="Property">
          <select style={s.select} value={kfProp} onChange={(e) => setKfProp(e.target.value as any)}>
            <option value="position">Position</option>
            <option value="rotation">Rotation</option>
            <option value="scale">Scale</option>
            <option value="opacity">Opacity</option>
          </select>
        </Row>
        <Row label="Easing">
          <select style={s.select} value={easing} onChange={(e) => setEasing(e.target.value)}>
            <option value="linear">Linear</option>
            <option value="ease-in">Ease In</option>
            <option value="ease-out">Ease Out</option>
            <option value="ease-in-out">Ease In-Out</option>
            <option value="step">Step</option>
          </select>
        </Row>
        <div style={{ padding: '6px 8px' }}>
          <button style={s.kfBtn} onClick={insertKeyframe}>◆ Insert Keyframe @ {frame}</button>
        </div>
      </Section>

      <Section title="Keyframes">
        <div style={{ maxHeight: 130, overflowY: 'auto' }}>
          {el.tracks.length === 0 && <div style={{ padding: '8px 12px', color: '#333344', fontSize: 11 }}>None yet</div>}
          {el.tracks.flatMap((nt) =>
            nt.track.keys.map((k, i) => (
              <div key={`${nt.property}-${i}`} style={s.kfRow}>
                <span style={s.kfFrame}>{Math.round(k.frame)}</span>
                <span style={s.kfProp}>{nt.property}</span>
                <span style={s.kfEase}>{k.easing}</span>
              </div>
            ))
          )}
        </div>
      </Section>
    </div>
  )
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div style={{ borderBottom: '1px solid #1a1a22' }}>
      <div style={s.sectionTitle}>{title}</div>
      {children}
    </div>
  )
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div style={s.row}>
      <span style={s.label}>{label}</span>
      <div style={s.control}>{children}</div>
    </div>
  )
}

function Vec3({ label, value, onChange, step }: { label: string; value: [number, number, number]; onChange: (v: [number, number, number]) => void; step: number }) {
  return (
    <div style={{ padding: '2px 8px' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
        <span style={{ ...s.label, width: 28 }}>{label}</span>
        {(['x', 'y', 'z'] as const).map((axis, i) => (
          <div key={axis} style={s.vecField}>
            <span style={{ ...s.axis, ...(i === 0 ? s.ax : i === 1 ? s.ay : s.az) }}>{axis.toUpperCase()}</span>
            <input type="number" step={step} style={s.numInput} value={value[i].toFixed(3)}
              onChange={(e) => { const n = [...value] as [number, number, number]; n[i] = parseFloat(e.target.value) || 0; onChange(n) }} />
          </div>
        ))}
      </div>
    </div>
  )
}

function Num({ value, onChange }: { value: number; onChange: (v: number) => void }) {
  return <input type="number" step={0.1} style={{ ...s.numInput, background: '#1a1a22', border: '1px solid #222232', borderRadius: 4, width: '100%' }} value={value.toFixed(2)} onChange={(e) => onChange(parseFloat(e.target.value) || 0)} />
}

const s: Record<string, React.CSSProperties> = {
  panel: { display: 'flex', flexDirection: 'column', height: '100%', background: '#111116', color: '#c8c8d4', fontSize: 12, fontFamily: "'Inter', sans-serif", overflowY: 'auto' },
  header: { padding: '7px 12px', fontSize: 10, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.09em', color: '#555570', background: '#0f0f14', borderBottom: '1px solid #1a1a22', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' },
  sectionTitle: { padding: '5px 12px', fontSize: 10, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.09em', color: '#5533bb' },
  row: { display: 'flex', alignItems: 'center', padding: '3px 8px', gap: 8 },
  label: { color: '#555570', width: 50, flexShrink: 0, fontSize: 11 },
  control: { flex: 1, display: 'flex', alignItems: 'center', gap: 6 },
  val: { color: '#8888aa', fontFamily: 'monospace', fontSize: 11 },
  input: { background: '#1a1a22', border: '1px solid #222232', borderRadius: 4, color: '#c8c8d4', padding: '5px 8px', fontSize: 12, outline: 'none', width: 'calc(100% - 16px)', margin: '2px 8px' },
  select: { background: '#1a1a22', border: '1px solid #222232', borderRadius: 4, color: '#c8c8d4', padding: '3px 6px', fontSize: 11, outline: 'none', width: '100%' },
  color: { width: 32, height: 22, border: 'none', background: 'none', cursor: 'pointer' },
  kfBtn: { width: '100%', padding: '7px 0', background: '#5533bb', border: 'none', borderRadius: 5, color: 'white', fontSize: 11, fontWeight: 600, cursor: 'pointer' },
  kfRow: { display: 'flex', alignItems: 'center', padding: '2px 12px', gap: 8, borderBottom: '1px solid #16161e' },
  kfFrame: { color: '#6644ff', fontFamily: 'monospace', fontSize: 10, minWidth: 28 },
  kfProp: { flex: 1, color: '#888899', fontSize: 10 },
  kfEase: { color: '#444460', fontSize: 9 },
  vecField: { display: 'flex', flex: 1, alignItems: 'center', background: '#1a1a22', border: '1px solid #222232', borderRadius: 4, overflow: 'hidden' },
  axis: { padding: '2px 4px', fontSize: 9, fontWeight: 700, flexShrink: 0 },
  ax: { color: '#ff4455', background: 'rgba(255,68,85,0.12)' },
  ay: { color: '#44ff88', background: 'rgba(68,255,136,0.12)' },
  az: { color: '#4488ff', background: 'rgba(68,136,255,0.12)' },
  numInput: { background: 'transparent', border: 'none', color: '#c8c8d4', padding: '2px 4px', fontSize: 10, width: '100%', textAlign: 'right', outline: 'none', fontFamily: 'monospace' },
}
