import { Play, Pause, SkipBack, Video } from 'lucide-react'
import { useSceneStore } from './store/scene'
import { mainSheet } from './store/theatre'
import './Toolbar.css'

export function Toolbar() {
  const isPlaying = useSceneStore((s) => s.isPlaying)
  const playhead = useSceneStore((s) => s.playhead)
  const duration = useSceneStore((s) => s.duration)
  const setPlaying = useSceneStore((s) => s.setPlaying)
  const setPlayhead = useSceneStore((s) => s.setPlayhead)

  const togglePlay = () => {
    if (isPlaying) {
      mainSheet.sequence.pause()
      setPlaying(false)
    } else {
      mainSheet.sequence.play({ iterationCount: 1 })
      setPlaying(true)
    }
  }

  const rewind = () => {
    mainSheet.sequence.position = 0
    setPlayhead(0)
    setPlaying(false)
  }

  return (
    <div className="toolbar">
      <div className="toolbar-brand">
        <span className="toolbar-logo">⚡ Juicer</span>
        <span className="toolbar-tagline">Product Demo Studio</span>
      </div>

      <div className="toolbar-playback">
        <button className="tb-btn" onClick={rewind} title="Rewind to start">
          <SkipBack size={14} />
        </button>
        <button className={`tb-btn play ${isPlaying ? 'playing' : ''}`} onClick={togglePlay}>
          {isPlaying ? <Pause size={16} /> : <Play size={16} />}
        </button>
        <span className="tb-time">
          {playhead.toFixed(2)}s / {duration}s
        </span>
      </div>

      <div className="toolbar-right">
        <span className="tb-hint">Theatre.js timeline ↓</span>
      </div>
    </div>
  )
}
