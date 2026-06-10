import { useSceneStore, useSelectedLayer } from '../store/scene'

export function Properties() {
  const layer = useSelectedLayer()
  const frame = useSceneStore((s) => s.frame)
  const setTransform = useSceneStore((s) => s.setTransform)
  const setOpacity = useSceneStore((s) => s.setOpacity)
  const setSize = useSceneStore((s) => s.setSize)
  const setBorderRadius = useSceneStore((s) => s.setBorderRadius)
  const setShadow = useSceneStore((s) => s.setShadow)
  const clearShadows = useSceneStore((s) => s.clearShadows)
  const setBlur = useSceneStore((s) => s.setBlur)
  const setKeyframe = useSceneStore((s) => s.setKeyframe)
  const renameLayer = useSceneStore((s) => s.renameLayer)

  if (!layer) {
    return (
      <div style={styles.empty}>
        Select a layer to edit its transform, fill, shadow, and effects.
      </div>
    )
  }

  const t = layer.transform
  const shadow = layer.effects.shadows[0]

  return (
    <div style={styles.root}>
      <div style={styles.header}>
        <input
          style={styles.nameInput}
          value={layer.name}
          onChange={(e) => renameLayer(layer.id, e.target.value)}
        />
        <span style={styles.id}>{layer.id}</span>
      </div>

      <Section title="Transform">
        <Row>
          <Num label="X" value={t.x} onChange={(v) => setTransform(layer.id, { x: v })}
               onKey={() => setKeyframe(layer.id, frame, 'x', t.x)} />
          <Num label="Y" value={t.y} onChange={(v) => setTransform(layer.id, { y: v })}
               onKey={() => setKeyframe(layer.id, frame, 'y', t.y)} />
        </Row>
        <Row>
          <Num label="W" value={layer.width} onChange={(v) => setSize(layer.id, v)}
               onKey={() => setKeyframe(layer.id, frame, 'width', layer.width)} />
          <Num label="H" value={layer.height} onChange={(v) => setSize(layer.id, undefined, v)}
               onKey={() => setKeyframe(layer.id, frame, 'height', layer.height)} />
        </Row>
        <Row>
          <Num label="Rot°" value={t.rotation} onChange={(v) => setTransform(layer.id, { rotation: v })}
               onKey={() => setKeyframe(layer.id, frame, 'rotation', t.rotation)} />
          <Num label="Scale X" value={t.scale_x} onChange={(v) => setTransform(layer.id, { scale_x: v })}
               onKey={() => setKeyframe(layer.id, frame, 'scale_x', t.scale_x)} step={0.1} />
          <Num label="Scale Y" value={t.scale_y} onChange={(v) => setTransform(layer.id, { scale_y: v })}
               onKey={() => setKeyframe(layer.id, frame, 'scale_y', t.scale_y)} step={0.1} />
        </Row>
      </Section>

      <Section title="2.5D">
        <Row>
          <Num label="Rot X°" value={t.rotate_x} onChange={(v) => setTransform(layer.id, { rotate_x: v })}
               onKey={() => setKeyframe(layer.id, frame, 'rotate_x', t.rotate_x)} />
          <Num label="Rot Y°" value={t.rotate_y} onChange={(v) => setTransform(layer.id, { rotate_y: v })}
               onKey={() => setKeyframe(layer.id, frame, 'rotate_y', t.rotate_y)} />
          <Num label="Persp" value={t.perspective} onChange={(v) => setTransform(layer.id, { perspective: v })}
               onKey={() => setKeyframe(layer.id, frame, 'perspective', t.perspective)} />
        </Row>
      </Section>

      <Section title="Appearance">
        <Row>
          <Num label="Opacity" value={layer.opacity} min={0} max={1} step={0.05}
               onChange={(v) => setOpacity(layer.id, v)}
               onKey={() => setKeyframe(layer.id, frame, 'opacity', layer.opacity)} />
          <Num label="Radius" value={((layer.kind as any).border_radius as number | undefined) ?? 0}
               onChange={(v) => setBorderRadius(layer.id, v)}
               onKey={() => setKeyframe(layer.id, frame, 'border_radius',
                 ((layer.kind as any).border_radius as number | undefined) ?? 0)} />
        </Row>
      </Section>

      <Section title="Shadow">
        <Row>
          <Num label="Offset X" value={shadow?.offset_x ?? 0}
               onChange={(v) => setShadow(layer.id, { ...defaultShadow(shadow), offset_x: v })}
               onKey={() => setKeyframe(layer.id, frame, 'shadow_offset_x', shadow?.offset_x ?? 0)} />
          <Num label="Offset Y" value={shadow?.offset_y ?? 8}
               onChange={(v) => setShadow(layer.id, { ...defaultShadow(shadow), offset_y: v })}
               onKey={() => setKeyframe(layer.id, frame, 'shadow_offset_y', shadow?.offset_y ?? 8)} />
        </Row>
        <Row>
          <Num label="Blur" value={shadow?.blur ?? 32}
               onChange={(v) => setShadow(layer.id, { ...defaultShadow(shadow), blur: v })}
               onKey={() => setKeyframe(layer.id, frame, 'shadow_blur', shadow?.blur ?? 32)} />
          <Num label="Spread" value={shadow?.spread ?? 0}
               onChange={(v) => setShadow(layer.id, { ...defaultShadow(shadow), spread: v })}
               onKey={() => setKeyframe(layer.id, frame, 'shadow_spread', shadow?.spread ?? 0)} />
        </Row>
        <Row>
          <ColorInput label="Color" value={shadow?.color ?? 'rgba(0,0,0,0.35)'}
                     onChange={(v) => setShadow(layer.id, { ...defaultShadow(shadow), color: v })} />
          <button style={styles.btn} onClick={() => clearShadows(layer.id)}>Clear</button>
        </Row>
      </Section>

      <Section title="Filter">
        <Row>
          <Num label="Blur (px)" value={layer.effects.filter_blur}
               onChange={(v) => setBlur(layer.id, v)}
               onKey={() => setKeyframe(layer.id, frame, 'filter_blur', layer.effects.filter_blur)} />
        </Row>
      </Section>
    </div>
  )
}

function defaultShadow(s?: { offset_x: number; offset_y: number; blur: number; spread: number; color: string }) {
  return {
    offset_x: s?.offset_x ?? 0,
    offset_y: s?.offset_y ?? 8,
    blur: s?.blur ?? 32,
    spread: s?.spread ?? 0,
    color: s?.color ?? 'rgba(0,0,0,0.35)',
  }
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div style={styles.section}>
      <div style={styles.sectionTitle}>{title}</div>
      <div style={styles.sectionBody}>{children}</div>
    </div>
  )
}

function Row({ children }: { children: React.ReactNode }) {
  return <div style={styles.row}>{children}</div>
}

function Num({
  label,
  value,
  onChange,
  onKey,
  min,
  max,
  step = 1,
}: {
  label: string
  value: number
  onChange: (v: number) => void
  onKey?: () => void
  min?: number
  max?: number
  step?: number
}) {
  return (
    <label style={styles.field}>
      <span style={styles.fieldLabel}>{label}</span>
      <input
        type="number"
        style={styles.input}
        value={value ?? 0}
        min={min}
        max={max}
        step={step}
        onChange={(e) => {
          const v = parseFloat(e.target.value)
          if (!Number.isNaN(v)) onChange(v)
        }}
      />
      {onKey && (
        <button title="Insert keyframe at current frame" style={styles.keyBtn} onClick={onKey}>
          ⬥
        </button>
      )}
    </label>
  )
}

function ColorInput({
  label,
  value,
  onChange,
}: {
  label: string
  value: string
  onChange: (v: string) => void
}) {
  // Try to coerce rgba()/hex into a #rrggbb for the color picker.
  const hex = toHex(value)
  return (
    <label style={styles.field}>
      <span style={styles.fieldLabel}>{label}</span>
      <input
        type="color"
        style={styles.colorInput}
        value={hex}
        onChange={(e) => onChange(e.target.value)}
      />
    </label>
  )
}

function toHex(v: string): string {
  if (v.startsWith('#') && v.length >= 7) return v.slice(0, 7)
  // Strip rgba(...) → #rrggbb, ignore alpha.
  const m = v.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/)
  if (m) {
    const [r, g, b] = [parseInt(m[1]), parseInt(m[2]), parseInt(m[3])]
    return '#' + [r, g, b].map((x) => x.toString(16).padStart(2, '0')).join('')
  }
  return '#000000'
}

const styles: Record<string, React.CSSProperties> = {
  root: {
    display: 'flex',
    flexDirection: 'column',
    height: '100%',
    background: '#15151c',
    borderLeft: '1px solid #2a2a36',
    overflowY: 'auto',
    color: '#cfcfdc',
    fontSize: 12,
  },
  empty: {
    padding: 16,
    color: '#666',
    fontSize: 12,
    fontStyle: 'italic',
  },
  header: {
    padding: 12,
    borderBottom: '1px solid #2a2a36',
    background: '#1a1a22',
    display: 'flex',
    flexDirection: 'column',
    gap: 4,
  },
  nameInput: {
    background: 'transparent',
    border: 0,
    color: '#fff',
    fontSize: 14,
    fontWeight: 500,
    outline: 'none',
    padding: 0,
  },
  id: {
    fontSize: 10,
    color: '#666',
    fontFamily: 'monospace',
  },
  section: {
    borderBottom: '1px solid #1c1c25',
  },
  sectionTitle: {
    fontSize: 10,
    fontWeight: 600,
    letterSpacing: 1,
    color: '#888',
    padding: '8px 12px 4px',
  },
  sectionBody: {
    padding: '0 12px 10px',
    display: 'flex',
    flexDirection: 'column',
    gap: 6,
  },
  row: {
    display: 'flex',
    gap: 8,
    flexWrap: 'wrap',
  },
  field: {
    flex: 1,
    minWidth: 70,
    display: 'flex',
    alignItems: 'center',
    gap: 4,
  },
  fieldLabel: {
    fontSize: 10,
    color: '#888',
    minWidth: 50,
  },
  input: {
    flex: 1,
    minWidth: 0,
    background: '#1f1f29',
    border: '1px solid #2a2a36',
    color: '#fff',
    padding: '3px 5px',
    borderRadius: 3,
    fontSize: 11,
    outline: 'none',
  },
  colorInput: {
    flex: 1,
    minWidth: 0,
    height: 22,
    background: '#1f1f29',
    border: '1px solid #2a2a36',
    borderRadius: 3,
    padding: 0,
    cursor: 'pointer',
  },
  keyBtn: {
    background: '#2a2a36',
    border: 0,
    color: '#ffaa66',
    padding: '2px 4px',
    borderRadius: 2,
    cursor: 'pointer',
    fontSize: 10,
  },
  btn: {
    background: '#1f1f29',
    border: '1px solid #2a2a36',
    color: '#cfcfdc',
    padding: '3px 8px',
    borderRadius: 3,
    cursor: 'pointer',
    fontSize: 11,
  },
}
