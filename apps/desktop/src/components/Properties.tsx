import { useSceneStore } from '../store/scene'
import { useState } from 'react'

export function Properties() {
  const objects = useSceneStore((s) => s.objects)
  const selected = useSceneStore((s) => s.selectedName)
  const cmd = useSceneStore((s) => s.cmd)
  const refresh = useSceneStore((s) => s.refreshScene)
  const frame = useSceneStore((s) => s.frame)

  const obj = objects.find((o) => o.name === selected)

  const [kfProp, setKfProp] = useState<'location' | 'rotation_euler' | 'scale' | 'alpha'>('location')

  const update = async (field: string, value: unknown) => {
    if (!obj) return
    await cmd('update_object', { name: obj.name, [field]: value })
    refresh()
  }

  const addKeyframe = async () => {
    if (!obj) return
    let value: unknown
    if (kfProp === 'location') value = obj.location
    else if (kfProp === 'rotation_euler') value = obj.rotation
    else if (kfProp === 'scale') value = obj.scale
    else value = 1.0
    await cmd('set_keyframe', { name: obj.name, frame, property: kfProp, value })
    refresh()
  }

  if (!obj) {
    return (
      <div style={{ ...s.panel, justifyContent: 'center', alignItems: 'center', color: '#333344' }}>
        <div style={{ textAlign: 'center', fontSize: 12 }}>Select an object</div>
      </div>
    )
  }

  return (
    <div style={s.panel}>
      <div style={s.header}>Properties — {obj.name}</div>

      <Section title="Transform">
        <Vec3Row label="Loc" value={obj.location} onChange={(v) => update('location', v)} />
        <Vec3Row label="Rot" value={obj.rotation} onChange={(v) => update('rotation', v)} />
        <Vec3Row label="Scl" value={obj.scale} onChange={(v) => update('scale', v)} />
      </Section>

      <Section title="Keyframe">
        <Row label="Frame">
          <span style={s.value}>{frame}</span>
        </Row>
        <Row label="Property">
          <select
            style={s.select}
            value={kfProp}
            onChange={(e) => setKfProp(e.target.value as any)}
          >
            <option value="location">Location</option>
            <option value="rotation_euler">Rotation</option>
            <option value="scale">Scale</option>
            <option value="alpha">Opacity</option>
          </select>
        </Row>
        <div style={{ padding: '6px 8px' }}>
          <button style={s.kfBtn} onClick={addKeyframe}>
            ◆ Insert Keyframe at frame {frame}
          </button>
        </div>
      </Section>

      <Section title="Keyframes on object">
        <div style={{ maxHeight: 120, overflowY: 'auto' }}>
          {obj.keyframes.length === 0 && (
            <div style={{ padding: '8px 12px', color: '#333344', fontSize: 11 }}>None</div>
          )}
          {obj.keyframes.map((kf, i) => (
            <div key={i} style={s.kfRow}>
              <span style={s.kfFrame}>{Math.round(kf.frame)}</span>
              <span style={s.kfPath}>{kf.data_path}[{kf.array_index}]</span>
              <span style={s.kfVal}>{kf.value.toFixed(3)}</span>
            </div>
          ))}
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

function Vec3Row({
  label,
  value,
  onChange,
}: {
  label: string
  value: [number, number, number]
  onChange: (v: [number, number, number]) => void
}) {
  return (
    <div style={{ padding: '2px 8px' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
        <span style={{ ...s.label, width: 28 }}>{label}</span>
        {(['x', 'y', 'z'] as const).map((axis, i) => (
          <div key={axis} style={s.vecField}>
            <span style={{ ...s.axis, ...(i === 0 ? s.axisX : i === 1 ? s.axisY : s.axisZ) }}>
              {axis.toUpperCase()}
            </span>
            <input
              type="number"
              style={s.numInput}
              value={value[i].toFixed(3)}
              step={0.1}
              onChange={(e) => {
                const next = [...value] as [number, number, number]
                next[i] = parseFloat(e.target.value) || 0
                onChange(next)
              }}
            />
          </div>
        ))}
      </div>
    </div>
  )
}

const s: Record<string, React.CSSProperties> = {
  panel: {
    display: 'flex',
    flexDirection: 'column',
    height: '100%',
    background: '#111116',
    color: '#c8c8d4',
    fontSize: 12,
    fontFamily: "'Inter', 'Segoe UI', sans-serif",
    overflowY: 'auto',
  },
  header: {
    padding: '7px 12px',
    fontSize: 10,
    fontWeight: 700,
    textTransform: 'uppercase',
    letterSpacing: '0.09em',
    color: '#555570',
    background: '#0f0f14',
    borderBottom: '1px solid #1a1a22',
    whiteSpace: 'nowrap',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
  },
  sectionTitle: {
    padding: '5px 12px',
    fontSize: 10,
    fontWeight: 700,
    textTransform: 'uppercase',
    letterSpacing: '0.09em',
    color: '#5533bb',
  },
  row: {
    display: 'flex',
    alignItems: 'center',
    padding: '3px 8px',
    gap: 8,
  },
  label: { color: '#555570', width: 50, flexShrink: 0, fontSize: 11 },
  control: { flex: 1 },
  value: { color: '#8888aa', fontFamily: 'monospace', fontSize: 11 },
  select: {
    background: '#1a1a22',
    border: '1px solid #222232',
    borderRadius: 4,
    color: '#c8c8d4',
    padding: '3px 6px',
    fontSize: 11,
    outline: 'none',
    width: '100%',
  },
  kfBtn: {
    width: '100%',
    padding: '7px 0',
    background: '#5533bb',
    border: 'none',
    borderRadius: 5,
    color: 'white',
    fontSize: 11,
    fontWeight: 600,
    cursor: 'pointer',
    letterSpacing: '0.02em',
  },
  kfRow: {
    display: 'flex',
    alignItems: 'center',
    padding: '2px 12px',
    gap: 8,
    borderBottom: '1px solid #16161e',
  },
  kfFrame: { color: '#6644ff', fontFamily: 'monospace', fontSize: 10, minWidth: 30 },
  kfPath: { flex: 1, color: '#555570', fontSize: 10, overflow: 'hidden', textOverflow: 'ellipsis' },
  kfVal: { color: '#888899', fontFamily: 'monospace', fontSize: 10 },
  vecField: {
    display: 'flex',
    flex: 1,
    alignItems: 'center',
    background: '#1a1a22',
    border: '1px solid #222232',
    borderRadius: 4,
    overflow: 'hidden',
  },
  axis: { padding: '2px 4px', fontSize: 9, fontWeight: 700, flexShrink: 0 },
  axisX: { color: '#ff4455', background: 'rgba(255,68,85,0.12)' },
  axisY: { color: '#44ff88', background: 'rgba(68,255,136,0.12)' },
  axisZ: { color: '#4488ff', background: 'rgba(68,136,255,0.12)' },
  numInput: {
    background: 'transparent',
    border: 'none',
    color: '#c8c8d4',
    padding: '2px 4px',
    fontSize: 10,
    width: '100%',
    textAlign: 'right',
    outline: 'none',
    fontFamily: 'monospace',
  },
}
