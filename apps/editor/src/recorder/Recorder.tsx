import { useState, useRef } from 'react'
import { useSceneStore } from '../store/scene'
import { Video, Square, Download, Settings } from 'lucide-react'
import './Recorder.css'

export function Recorder() {
  const recording = useSceneStore((s) => s.recording)
  const setRecording = useSceneStore((s) => s.setRecording)
  const setPlaying = useSceneStore((s) => s.setPlaying)
  const setPlayhead = useSceneStore((s) => s.setPlayhead)
  const duration = useSceneStore((s) => s.duration)

  const mediaRecorderRef = useRef<MediaRecorder | null>(null)
  const chunksRef = useRef<Blob[]>([])
  const [status, setStatus] = useState<'idle' | 'recording' | 'processing'>('idle')

  const startRecording = async () => {
    const canvas = document.querySelector('canvas')
    if (!canvas) return

    const stream = canvas.captureStream(recording.fps)
    const mr = new MediaRecorder(stream, {
      mimeType: 'video/webm;codecs=vp9',
      videoBitsPerSecond: 8_000_000,
    })
    chunksRef.current = []
    mr.ondataavailable = (e) => {
      if (e.data.size > 0) chunksRef.current.push(e.data)
    }
    mr.onstop = () => {
      setStatus('processing')
      const blob = new Blob(chunksRef.current, { type: 'video/webm' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `juicer-${Date.now()}.webm`
      a.click()
      URL.revokeObjectURL(url)
      setStatus('idle')
      setRecording({ isRecording: false })
    }

    mediaRecorderRef.current = mr
    mr.start(100)
    setRecording({ isRecording: true })
    setStatus('recording')
    setPlayhead(0)
    setPlaying(true)

    // Auto stop after duration
    setTimeout(() => {
      stopRecording()
    }, recording.duration * 1000 + 500)
  }

  const stopRecording = () => {
    if (mediaRecorderRef.current?.state === 'recording') {
      mediaRecorderRef.current.stop()
      setPlaying(false)
    }
  }

  return (
    <div className="recorder">
      <div className="recorder-header">
        <Video size={14} />
        <span>Export</span>
      </div>

      <div className="recorder-body">
        <div className="recorder-row">
          <span className="recorder-label">FPS</span>
          <select
            className="recorder-select"
            value={recording.fps}
            onChange={(e) => setRecording({ fps: parseInt(e.target.value) })}
            disabled={status === 'recording'}
          >
            <option value={24}>24</option>
            <option value={30}>30</option>
            <option value={60}>60</option>
          </select>
        </div>

        <div className="recorder-row">
          <span className="recorder-label">Duration</span>
          <input
            type="number"
            className="recorder-num"
            value={recording.duration}
            min={1} max={120} step={1}
            onChange={(e) => setRecording({ duration: parseInt(e.target.value) || 10 })}
            disabled={status === 'recording'}
          />
          <span className="recorder-unit">sec</span>
        </div>

        <div className="recorder-row">
          <span className="recorder-label">Format</span>
          <span className="recorder-note">WebM (VP9)</span>
        </div>

        {status === 'recording' ? (
          <button className="recorder-btn stop" onClick={stopRecording}>
            <Square size={14} /> Stop
          </button>
        ) : status === 'processing' ? (
          <button className="recorder-btn processing" disabled>
            Processing...
          </button>
        ) : (
          <button className="recorder-btn start" onClick={startRecording}>
            <Video size={14} /> Record
          </button>
        )}

        {status === 'recording' && (
          <div className="recorder-indicator">
            <span className="recorder-dot" />
            Recording...
          </div>
        )}

        <p className="recorder-hint">
          Records the 3D canvas. Theatre.js will play your animation while recording.
        </p>
      </div>
    </div>
  )
}
