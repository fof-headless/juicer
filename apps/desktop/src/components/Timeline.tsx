import { useMemo, useRef } from 'react'
import { useSceneStore, useSelectedLayer } from '../store/scene'

/// Horizontal timeline strip: frame ruler, draggable playhead, and per-track
/// keyframe dots for the selected layer. Scrubbing seeks the preview.
export function Timeline() {
  const scene = useSceneStore((s) => s.scene)
  const frame = useSceneStore((s) => s.frame)
  const setFrame = useSceneStore((s) => s.setFrame)
  const layer = useSelectedLayer()
  const trackRef = useRef<HTMLDivElement>(null)

  const duration = scene?.duration_frames ?? 1
  const fps = scene?.canvas.fps ?? 30
  const tickEveryN = useMemo(() => {
    // Aim for ~10 labels across.
    const target = Math.max(1, Math.round(duration / 10))
    return target
  }, [duration])

  const xForFrame = (f: number) => (f / Math.max(1, duration - 1)) * 100

  const onScrub = (e: React.MouseEvent | React.PointerEvent) => {
    const el = trackRef.current
    if (!el) return
    const rect = el.getBoundingClientRect()
    const ratio = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width))
    const f = Math.round(ratio * (duration - 1))
    setFrame(f)
  }

  return (
    <div style={styles.root}>
      <div style={styles.header}>
        <span>TIMELINE</span>
        <span style={{ marginLeft: 'auto' }}>
          {frame} / {duration} • {(frame / fps).toFixed(2)}s
        </span>
      </div>

      <div
        ref={trackRef}
        style={styles.track}
        onPointerDown={(e) => {
          (e.target as HTMLElement).setPointerCapture?.(e.pointerId)
          onScrub(e)
        }}
        onPointerMove={(e) => {
          if (e.buttons === 1) onScrub(e)
        }}
      >
        {/* Ruler ticks */}
        {Array.from({ length: Math.ceil(duration / tickEveryN) + 1 }).map((_, i) => {
          const f = i * tickEveryN
          if (f > duration) return null
          return (
            <div key={i} style={{ ...styles.tick, left: `${xForFrame(f)}%` }}>
              <div style={styles.tickMark} />
              <div style={styles.tickLabel}>{f}</div>
            </div>
          )
        })}

        {/* Keyframe markers for selected layer */}
        {layer &&
          layer.tracks.flatMap((nt) =>
            nt.track.keys.map((k, i) => (
              <div
                key={`${nt.property}-${i}`}
                title={`${nt.property} @ frame ${k.frame}`}
                style={{
                  ...styles.kfDot,
                  left: `${xForFrame(k.frame)}%`,
                  background: keyframeColor(nt.property),
                }}
              />
            )),
          )}

        {/* Playhead */}
        <div style={{ ...styles.playhead, left: `${xForFrame(frame)}%` }} />
      </div>

      {layer && layer.tracks.length > 0 && (
        <div style={styles.tracksList}>
          {layer.tracks.map((nt) => (
            <div key={nt.property} style={styles.trackRow}>
              <span style={styles.trackLabel}>{nt.property}</span>
              <span style={{ ...styles.kfDot, position: 'relative', background: keyframeColor(nt.property), marginRight: 8 }} />
              <span style={styles.trackCount}>{nt.track.keys.length} kf</span>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

function keyframeColor(property: string): string {
  // Stable color per property name.
  let h = 0
  for (let i = 0; i < property.length; i++) h = (h * 31 + property.charCodeAt(i)) >>> 0
  return `hsl(${h % 360} 70% 55%)`
}

const styles: Record<string, React.CSSProperties> = {
  root: {
    display: 'flex',
    flexDirection: 'column',
    height: '100%',
    background: '#15151c',
    borderTop: '1px solid #2a2a36',
    overflow: 'hidden',
    fontSize: 11,
    color: '#cfcfdc',
  },
  header: {
    padding: '6px 12px',
    fontSize: 10,
    fontWeight: 600,
    letterSpacing: 1,
    color: '#888',
    borderBottom: '1px solid #2a2a36',
    display: 'flex',
    alignItems: 'center',
  },
  track: {
    position: 'relative',
    height: 38,
    background: '#181820',
    margin: '6px 12px',
    borderRadius: 3,
    overflow: 'hidden',
    cursor: 'pointer',
    userSelect: 'none',
  },
  tick: {
    position: 'absolute',
    top: 0,
    bottom: 0,
    transform: 'translateX(-0.5px)',
  },
  tickMark: {
    width: 1,
    height: 4,
    background: '#3a3a4a',
  },
  tickLabel: {
    fontSize: 9,
    color: '#666',
    marginTop: 2,
    transform: 'translateX(-50%)',
  },
  kfDot: {
    position: 'absolute',
    top: '50%',
    width: 8,
    height: 8,
    marginLeft: -4,
    marginTop: -4,
    borderRadius: 2,
    transform: 'rotate(45deg)',
    boxShadow: '0 0 0 1px #15151c',
  },
  playhead: {
    position: 'absolute',
    top: 0,
    bottom: 0,
    width: 1,
    background: '#ffaa66',
    boxShadow: '0 0 4px rgba(255,170,102,0.6)',
    pointerEvents: 'none',
  },
  tracksList: {
    overflowY: 'auto',
    padding: '0 12px 8px',
  },
  trackRow: {
    display: 'flex',
    alignItems: 'center',
    padding: '4px 6px',
    fontSize: 11,
    color: '#aaa',
  },
  trackLabel: {
    flex: 1,
  },
  trackCount: {
    fontSize: 10,
    color: '#666',
  },
}
