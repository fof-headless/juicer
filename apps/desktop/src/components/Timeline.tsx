import { useMemo, useRef } from 'react'
import { useSceneStore } from '../store/scene'

/// A Blender-style timeline strip: frame ruler, draggable playhead, and
/// keyframe dots for the selected element's tracks. Scrubbing seeks + previews.
export function Timeline() {
  const scene = useSceneStore((s) => s.scene)
  const frame = useSceneStore((s) => s.frame)
  const setFrame = useSceneStore((s) => s.setFrame)
  const renderPreview = useSceneStore((s) => s.renderPreview)
  const selectedId = useSceneStore((s) => s.selectedId)
  const trackRef = useRef<HTMLDivElement>(null)

  const start = scene?.render.frame_start ?? 1
  const end = Math.max(scene?.render.frame_end ?? 300, start + 1)
  const span = end - start

  const selected = scene?.elements.find((e) => e.id === selectedId || e.name === selectedId)

  // Collect keyframes per property for the selected element.
  const keyframes = useMemo(() => {
    if (!selected) return [] as { frame: number; property: string }[]
    const out: { frame: number; property: string }[] = []
    for (const t of selected.tracks ?? []) {
      for (const k of t.track?.keys ?? []) out.push({ frame: k.frame, property: t.property })
    }
    return out
  }, [selected])

  const pct = (f: number) => ((f - start) / span) * 100

  const seekFromClientX = (clientX: number) => {
    const el = trackRef.current
    if (!el) return
    const rect = el.getBoundingClientRect()
    const ratio = Math.min(1, Math.max(0, (clientX - rect.left) / rect.width))
    const f = Math.round(start + ratio * span)
    setFrame(f)
    renderPreview(f)
  }

  const onPointerDown = (e: React.PointerEvent) => {
    ;(e.target as HTMLElement).setPointerCapture?.(e.pointerId)
    seekFromClientX(e.clientX)
  }
  const onPointerMove = (e: React.PointerEvent) => {
    if (e.buttons !== 1) return
    seekFromClientX(e.clientX)
  }

  // Ruler ticks every ~10% of the span.
  const ticks = useMemo(() => {
    const n = 10
    return Array.from({ length: n + 1 }, (_, i) => start + Math.round((span * i) / n))
  }, [start, span])

  return (
    <div style={s.wrap}>
      <div style={s.head}>
        <span style={s.label}>Timeline</span>
        <span style={s.range}>{start}–{end}</span>
        {selected && <span style={s.sel}>● {selected.name}</span>}
        <span style={s.frameTag}>frame {frame}</span>
      </div>

      <div
        ref={trackRef}
        style={s.track}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
      >
        {/* ruler ticks */}
        {ticks.map((t, i) => (
          <div key={i} style={{ ...s.tick, left: `${pct(t)}%` }}>
            <span style={s.tickLabel}>{t}</span>
          </div>
        ))}

        {/* keyframe dots */}
        {keyframes.map((k, i) => (
          <div
            key={i}
            title={`${k.property} @ ${k.frame}`}
            style={{ ...s.key, left: `${pct(k.frame)}%` }}
          />
        ))}

        {/* playhead */}
        <div style={{ ...s.playhead, left: `${pct(frame)}%` }}>
          <div style={s.playheadKnob} />
        </div>
      </div>
    </div>
  )
}

const s: Record<string, React.CSSProperties> = {
  wrap: { display: 'flex', flexDirection: 'column', height: '100%', background: '#0c0c12', borderTop: '1px solid #1a1a22' },
  head: { display: 'flex', alignItems: 'center', gap: 12, padding: '5px 12px', borderBottom: '1px solid #15151d' },
  label: { fontSize: 10, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.09em', color: '#555570' },
  range: { fontSize: 10, color: '#333348', fontFamily: 'monospace' },
  sel: { fontSize: 11, color: '#9977ff', fontWeight: 600 },
  frameTag: { marginLeft: 'auto', fontSize: 11, color: '#7777aa', fontFamily: 'monospace' },
  track: { position: 'relative', flex: 1, margin: '10px 12px 14px', background: '#111119', borderRadius: 6, border: '1px solid #1a1a24', cursor: 'pointer', minHeight: 44 },
  tick: { position: 'absolute', top: 0, bottom: 0, width: 1, background: '#1a1a26' },
  tickLabel: { position: 'absolute', top: 2, left: 3, fontSize: 9, color: '#33334a', fontFamily: 'monospace' },
  key: { position: 'absolute', bottom: 8, width: 9, height: 9, marginLeft: -4.5, background: '#ffaa33', borderRadius: 2, transform: 'rotate(45deg)', border: '1px solid #cc7711', boxShadow: '0 0 4px rgba(255,170,51,0.5)' },
  playhead: { position: 'absolute', top: 0, bottom: 0, width: 2, marginLeft: -1, background: '#cc3355', pointerEvents: 'none' },
  playheadKnob: { position: 'absolute', top: -1, left: -4, width: 10, height: 10, background: '#cc3355', borderRadius: '50%' },
}
