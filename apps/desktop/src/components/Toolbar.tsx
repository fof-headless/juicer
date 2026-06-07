import { useSceneStore } from '../store/scene'
import { Play, Pause, SkipBack, Video, Loader, Wifi, WifiOff, RefreshCw } from 'lucide-react'
import { useState } from 'react'

export function Toolbar() {
  const cmd = useSceneStore((s) => s.cmd)
  const refresh = useSceneStore((s) => s.refreshScene)
  const frame = useSceneStore((s) => s.frame)
  const frameEnd = useSceneStore((s) => s.frameEnd)
  const fps = useSceneStore((s) => s.fps)
  const connected = useSceneStore((s) => s.blenderConnected)
  const isRendering = useSceneStore((s) => s.isRendering)
  const setRendering = useSceneStore((s) => s.setRendering)

  const [playing, setPlaying] = useState(false)

  const rewind = async () => {
    await cmd('seek', { frame: 1 })
    setPlaying(false)
    refresh()
  }

  const togglePlay = async () => {
    if (playing) {
      await cmd('pause')
    } else {
      await cmd('seek', { frame: frame })
    }
    setPlaying(!playing)
  }

  const renderVideo = async () => {
    setRendering(true, 'Rendering with Eevee...')
    try {
      const home = (window as any).__TAURI__?.path?.homeDir?.() ?? '/tmp'
      const out = `/tmp/juicer_${Date.now()}`
      await cmd('render_animation', { output_path: out, start: 1, end: frameEnd, fps })
      setRendering(false, '')
      alert(`Render complete! Saved to: ${out}.mp4`)
    } catch (e: any) {
      setRendering(false, '')
      alert(`Render error: ${e?.message ?? e}`)
    }
  }

  const timecode = `${String(Math.floor(frame / fps / 60)).padStart(2, '0')}:${String(Math.floor((frame / fps) % 60)).padStart(2, '0')}:${String(frame).padStart(4, '0')}`

  return (
    <div style={s.bar}>
      <div style={s.brand}>
        <span style={s.logo}>⚡ Juicer</span>
      </div>

      <div style={s.playback}>
        <button style={s.btn} onClick={rewind} title="Rewind">
          <SkipBack size={13} />
        </button>
        <button style={{ ...s.btn, ...s.playBtn }} onClick={togglePlay} disabled={!connected}>
          {playing ? <Pause size={15} /> : <Play size={15} />}
        </button>
        <span style={s.timecode}>{timecode}</span>
      </div>

      <div style={s.right}>
        <button style={s.renderBtn} onClick={renderVideo} disabled={!connected || isRendering}>
          {isRendering ? <Loader size={13} /> : <Video size={13} />}
          {isRendering ? 'Rendering...' : 'Render'}
        </button>

        <button style={s.btn} onClick={refresh} title="Refresh scene">
          <RefreshCw size={12} />
        </button>

        <div style={{ ...s.status, ...(connected ? s.statusOk : s.statusBad) }}>
          {connected ? <Wifi size={11} /> : <WifiOff size={11} />}
          {connected ? 'Blender' : 'Offline'}
        </div>
      </div>
    </div>
  )
}

const s: Record<string, React.CSSProperties> = {
  bar: {
    display: 'flex',
    alignItems: 'center',
    height: 44,
    background: '#0a0a10',
    borderBottom: '1px solid #1a1a22',
    padding: '0 14px',
    gap: 16,
    flexShrink: 0,
    WebkitAppRegion: 'drag' as any,
    paddingLeft: 80, // space for macOS traffic lights
  },
  brand: {
    WebkitAppRegion: 'no-drag' as any,
  },
  logo: {
    fontWeight: 700,
    fontSize: 14,
    color: '#8866ff',
    letterSpacing: '-0.02em',
  },
  playback: {
    display: 'flex',
    alignItems: 'center',
    gap: 5,
    background: '#141420',
    padding: '4px 8px',
    borderRadius: 8,
    border: '1px solid #1e1e2e',
    WebkitAppRegion: 'no-drag' as any,
  },
  btn: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    background: 'none',
    border: 'none',
    color: '#7777aa',
    cursor: 'pointer',
    padding: 5,
    borderRadius: 4,
    transition: 'color 0.1s, background 0.1s',
  },
  playBtn: { color: '#c8c8d4' },
  timecode: {
    fontFamily: 'monospace',
    fontSize: 11,
    color: '#555570',
    minWidth: 90,
  },
  right: {
    marginLeft: 'auto',
    display: 'flex',
    alignItems: 'center',
    gap: 8,
    WebkitAppRegion: 'no-drag' as any,
  },
  renderBtn: {
    display: 'flex',
    alignItems: 'center',
    gap: 5,
    padding: '5px 12px',
    background: '#aa2233',
    border: 'none',
    borderRadius: 6,
    color: 'white',
    fontSize: 12,
    fontWeight: 600,
    cursor: 'pointer',
    transition: 'background 0.12s',
  },
  status: {
    display: 'flex',
    alignItems: 'center',
    gap: 5,
    padding: '4px 9px',
    borderRadius: 16,
    fontSize: 11,
    fontWeight: 600,
    border: '1px solid',
  },
  statusOk: { color: '#44cc77', borderColor: 'rgba(68,204,119,0.3)', background: 'rgba(68,204,119,0.06)' },
  statusBad: { color: '#cc4455', borderColor: 'rgba(204,68,85,0.3)', background: 'rgba(204,68,85,0.06)' },
}
