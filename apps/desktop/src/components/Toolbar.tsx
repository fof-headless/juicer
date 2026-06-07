import { useState } from 'react'
import { useSceneStore } from '../store/scene'
import { Play, Pause, SkipBack, Video, Loader, Image } from 'lucide-react'

export function Toolbar() {
  const scene = useSceneStore((s) => s.scene)
  const frame = useSceneStore((s) => s.frame)
  const setFrame = useSceneStore((s) => s.setFrame)
  const renderPreview = useSceneStore((s) => s.renderPreview)
  const renderVideo = useSceneStore((s) => s.renderVideo)
  const isRendering = useSceneStore((s) => s.isRendering)

  const [playing, setPlaying] = useState(false)
  const fps = scene?.render.fps ?? 30
  const frameEnd = scene?.render.frame_end ?? 300
  const frameStart = scene?.render.frame_start ?? 1

  // Simple JS playback loop: advances frame and re-renders preview.
  const togglePlay = () => {
    if (playing) { setPlaying(false); return }
    setPlaying(true)
    let f = frame
    const interval = setInterval(async () => {
      f += 1
      if (f > frameEnd) { f = frameStart }
      setFrame(f)
      await renderPreview(f)
      if (!(useSceneStore.getState() as any).__playing) { clearInterval(interval) }
    }, 1000 / fps)
    // store a flag via a custom prop
    ;(useSceneStore as any).setState({ __playing: true })
    const stop = () => { clearInterval(interval); ;(useSceneStore as any).setState({ __playing: false }) }
    ;(window as any).__juicerStop = stop
  }

  const stopPlay = () => {
    setPlaying(false)
    ;(window as any).__juicerStop?.()
  }

  const onPlayClick = () => {
    if (playing) stopPlay()
    else togglePlay()
  }

  const rewind = async () => { stopPlay(); setFrame(frameStart); await renderPreview(frameStart) }

  const doRender = async () => {
    const out = `/tmp/juicer_${Date.now()}`
    try {
      const path = await renderVideo(out)
      alert(`Render complete!\n${path}`)
    } catch (e: any) {
      alert(`Render error: ${e?.toString?.() ?? e}\n\nMake sure juicer-encoder is built (see README).`)
    }
  }

  const tc = `${String(Math.floor(frame / fps / 60)).padStart(2, '0')}:${String(Math.floor((frame / fps) % 60)).padStart(2, '0')} · f${frame}`

  return (
    <div style={s.bar}>
      <div style={s.brand}>
        <span style={s.logo}>⚡ Juicer</span>
        <span style={s.mode}>{(scene?.mode ?? 'lite').toUpperCase()}</span>
      </div>

      <div style={s.playback}>
        <button style={s.btn} onClick={rewind}><SkipBack size={13} /></button>
        <button style={{ ...s.btn, color: playing ? '#9977ff' : '#c8c8d4' }} onClick={onPlayClick}>
          {playing ? <Pause size={15} /> : <Play size={15} />}
        </button>
        <span style={s.tc}>{tc}</span>
      </div>

      <div style={s.right}>
        <button style={s.previewBtn} onClick={() => renderPreview()}>
          <Image size={12} /> Preview
        </button>
        <button style={s.renderBtn} onClick={doRender} disabled={isRendering}>
          {isRendering ? <Loader size={13} /> : <Video size={13} />}
          {isRendering ? 'Rendering…' : 'Render MP4'}
        </button>
      </div>
    </div>
  )
}

const s: Record<string, React.CSSProperties> = {
  bar: { display: 'flex', alignItems: 'center', height: 44, background: '#0a0a10', borderBottom: '1px solid #1a1a22', padding: '0 14px 0 80px', gap: 16, flexShrink: 0, ['WebkitAppRegion' as any]: 'drag' },
  brand: { display: 'flex', alignItems: 'baseline', gap: 8, ['WebkitAppRegion' as any]: 'no-drag' },
  logo: { fontWeight: 700, fontSize: 14, color: '#8866ff', letterSpacing: '-0.02em' },
  mode: { fontSize: 9, fontWeight: 700, color: '#44cc77', border: '1px solid rgba(68,204,119,0.3)', borderRadius: 4, padding: '1px 5px', letterSpacing: '0.08em' },
  playback: { display: 'flex', alignItems: 'center', gap: 5, background: '#141420', padding: '4px 8px', borderRadius: 8, border: '1px solid #1e1e2e', ['WebkitAppRegion' as any]: 'no-drag' },
  btn: { display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'none', border: 'none', color: '#7777aa', cursor: 'pointer', padding: 5, borderRadius: 4 },
  tc: { fontFamily: 'monospace', fontSize: 11, color: '#555570', minWidth: 90 },
  right: { marginLeft: 'auto', display: 'flex', alignItems: 'center', gap: 8, ['WebkitAppRegion' as any]: 'no-drag' },
  previewBtn: { display: 'flex', alignItems: 'center', gap: 5, padding: '5px 11px', background: '#1e1e2a', border: '1px solid #2a2a3a', borderRadius: 6, color: '#9988cc', fontSize: 12, fontWeight: 600, cursor: 'pointer' },
  renderBtn: { display: 'flex', alignItems: 'center', gap: 5, padding: '5px 12px', background: '#aa2233', border: 'none', borderRadius: 6, color: 'white', fontSize: 12, fontWeight: 600, cursor: 'pointer' },
}
