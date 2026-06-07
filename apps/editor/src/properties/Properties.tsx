import { useSceneStore } from '../store/scene'
import './Properties.css'

export function Properties() {
  const selectedIds = useSceneStore((s) => s.selectedIds)
  const elements = useSceneStore((s) => s.elements)
  const updateElement = useSceneStore((s) => s.updateElement)

  const el = selectedIds.length === 1 ? elements[selectedIds[0]] : null

  if (!el) {
    return (
      <div className="properties">
        <div className="properties-header">Properties</div>
        <div className="properties-empty">Select an element</div>
      </div>
    )
  }

  return (
    <div className="properties">
      <div className="properties-header">Properties — {el.name}</div>
      <div className="properties-body">

        <Section title="Name">
          <input
            className="prop-input"
            value={el.name}
            onChange={(e) => updateElement(el.id, { name: e.target.value })}
          />
        </Section>

        <Section title="Transform">
          <Vec3 label="Position" value={el.position} onChange={(v) => updateElement(el.id, { position: v })} step={0.1} />
          <Vec3 label="Rotation" value={el.rotation} onChange={(v) => updateElement(el.id, { rotation: v })} step={0.01} />
          <Vec3 label="Scale" value={el.scale} onChange={(v) => updateElement(el.id, { scale: v })} step={0.1} />
        </Section>

        <Section title="Appearance">
          <PropRow label="Opacity">
            <input
              type="range"
              min={0} max={1} step={0.01}
              value={el.opacity ?? 1}
              onChange={(e) => updateElement(el.id, { opacity: parseFloat(e.target.value) })}
              className="prop-range"
            />
            <span className="prop-value">{((el.opacity ?? 1) * 100).toFixed(0)}%</span>
          </PropRow>
          <PropRow label="Color">
            <input
              type="color"
              value={el.color ?? '#4488ff'}
              onChange={(e) => updateElement(el.id, { color: e.target.value })}
              className="prop-color"
            />
          </PropRow>
        </Section>

        {(el.type === 'html-plane' || el.type === 'image-plane' || el.type === 'box') && (
          <Section title="Size">
            <PropRow label="Width">
              <NumInput value={el.width ?? 4} onChange={(v) => updateElement(el.id, { width: v })} step={0.1} />
            </PropRow>
            <PropRow label="Height">
              <NumInput value={el.height ?? 2.5} onChange={(v) => updateElement(el.id, { height: v })} step={0.1} />
            </PropRow>
          </Section>
        )}

        {el.type === 'html-plane' && (
          <Section title="HTML Content">
            <textarea
              className="prop-textarea"
              value={el.htmlContent ?? ''}
              onChange={(e) => updateElement(el.id, { htmlContent: e.target.value })}
              rows={8}
              placeholder="Paste your HTML here..."
            />
          </Section>
        )}

        {el.type === 'text' && (
          <Section title="Text">
            <textarea
              className="prop-textarea"
              value={el.textContent ?? ''}
              onChange={(e) => updateElement(el.id, { textContent: e.target.value })}
              rows={3}
            />
          </Section>
        )}

        {el.type === 'image-plane' && (
          <Section title="Image URL">
            <input
              className="prop-input"
              value={el.imageUrl ?? ''}
              onChange={(e) => updateElement(el.id, { imageUrl: e.target.value })}
              placeholder="https://..."
            />
          </Section>
        )}

      </div>
    </div>
  )
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="prop-section">
      <div className="prop-section-title">{title}</div>
      {children}
    </div>
  )
}

function PropRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="prop-row">
      <span className="prop-label">{label}</span>
      <div className="prop-control">{children}</div>
    </div>
  )
}

function Vec3({
  label,
  value,
  onChange,
  step,
}: {
  label: string
  value: [number, number, number]
  onChange: (v: [number, number, number]) => void
  step: number
}) {
  return (
    <div className="prop-vec3">
      <span className="prop-label">{label}</span>
      <div className="prop-vec3-inputs">
        {(['x', 'y', 'z'] as const).map((axis, i) => (
          <div key={axis} className="prop-vec3-field">
            <span className={`prop-axis prop-axis-${axis}`}>{axis.toUpperCase()}</span>
            <NumInput
              value={value[i]}
              onChange={(v) => {
                const next = [...value] as [number, number, number]
                next[i] = v
                onChange(next)
              }}
              step={step}
            />
          </div>
        ))}
      </div>
    </div>
  )
}

function NumInput({ value, onChange, step = 0.1 }: { value: number; onChange: (v: number) => void; step?: number }) {
  return (
    <input
      type="number"
      className="prop-num"
      value={value.toFixed(3)}
      step={step}
      onChange={(e) => onChange(parseFloat(e.target.value) || 0)}
    />
  )
}
