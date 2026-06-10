import { useEffect, useState } from 'react'
import { useSceneStore } from '../store/scene'
import {
  Play,
  Pause,
  SkipBack,
  Video,
  Loader,
  Save,
  FolderPlus,
  ImagePlus,
} from 'lucide-react'
import { open as openFile } from '@tauri-apps/plugin-dialog'

export function Toolbar() {
  const scene = useSceneStore((s) => s.scene)
  const project = useSceneStore((s) => s.project)
  const refreshProject = useSceneStore((s) => s.refreshProject)
  const saveProject = useSceneStore((s) => s.saveProject)
  const createProject = useSceneStore((s) => s.createProject)
  const frame = useSceneStore((s) => s.frame)
  const setFrame = useSceneStore((s) => s.setFrame)
  const playing = useSceneStore((s) => s.playing)
  const setPlaying = useSceneStore((s) => s.setPlaying)
  const renderAnimation = useSceneStore((s) => s.renderAnimation)
  const isRendering = useSceneStore((s) => s.isRendering)
  const addImageLayer = useSceneStore((s) => s.addImageLayer)
  const refresh = useSceneStore((s) => s.refresh)

  useEffect(() => {
    refreshProject()
    refresh()
  }, [refreshProject, refresh])

  // Simple playback loop driven by setInterval based on fps.
  useEffect(() => {
    if (!playing || !scene) return
    const ms = 1000 / Math.max(1, scene.canvas.fps)
    const id = setInterval(() => {
      const next = (frame + 1) % Math.max(1, scene.duration_frames)
      setFrame(next)
    }, ms)
    return () => clearInterval(id)
  }, [playing, scene, frame, setFrame])

  const onAddImage = async () => {
    const picked = await openFile({
      multiple: false,
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg'] }],
    })
    if (typeof picked === 'string') {
      await addImageLayer(picked)
    }
  }

  const onNewProject = async () => {
    const name = window.prompt('New project name:', 'Untitled')
    if (name) await createProject(name)
  }

  const onRenderVideo = async () => {
    try {
      const path = await renderAnimation()
      alert(`Rendered:\n${path}`)
    } catch (e) {
      alert(`Render failed:\n${e}`)
    }
  }

  return (
    <div style={styles.root}>
      <div style={styles.left}>
        <span style={styles.brand}>Juicer</span>
        {project && <span style={styles.project}>{project.name}</span>}
      </div>

      <div style={styles.center}>
        <button style={styles.iconBtn} title="Rewind" onClick={() => setFrame(0)}>
          <SkipBack size={14} />
        </button>
        <button
          style={styles.iconBtn}
          title={playing ? 'Pause' : 'Play'}
          onClick={() => setPlaying(!playing)}
        >
          {playing ? <Pause size={14} /> : <Play size={14} />}
        </button>
      </div>

      <div style={styles.right}>
        <button style={styles.btn} onClick={onAddImage}>
          <ImagePlus size={12} /> Image
        </button>
        <button style={styles.btn} onClick={onNewProject}>
          <FolderPlus size={12} /> New project
        </button>
        <button style={styles.btn} onClick={() => saveProject()}>
          <Save size={12} /> Save
        </button>
        <button style={styles.primary} disabled={isRendering} onClick={onRenderVideo}>
          {isRendering ? <Loader size={12} /> : <Video size={12} />}{' '}
          {isRendering ? 'Rendering…' : 'Export MP4'}
        </button>
      </div>
    </div>
  )
}

const styles: Record<string, React.CSSProperties> = {
  root: {
    display: 'flex',
    alignItems: 'center',
    gap: 12,
    padding: '8px 12px',
    background: '#0d0d12',
    borderBottom: '1px solid #2a2a36',
    color: '#cfcfdc',
    fontSize: 12,
    // Leave room for the macOS traffic-light buttons under the overlay title bar.
    paddingLeft: 88,
    flexShrink: 0,
  },
  left: { display: 'flex', alignItems: 'center', gap: 12 },
  center: {
    flex: 1,
    display: 'flex',
    justifyContent: 'center',
    alignItems: 'center',
    gap: 6,
  },
  right: { display: 'flex', alignItems: 'center', gap: 6 },
  brand: { fontWeight: 600, color: '#fff' },
  project: { color: '#888' },
  iconBtn: {
    background: '#1f1f29',
    border: '1px solid #2a2a36',
    color: '#cfcfdc',
    padding: 5,
    borderRadius: 3,
    cursor: 'pointer',
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
  },
  btn: {
    background: '#1f1f29',
    border: '1px solid #2a2a36',
    color: '#cfcfdc',
    padding: '4px 8px',
    borderRadius: 3,
    cursor: 'pointer',
    fontSize: 11,
    display: 'inline-flex',
    alignItems: 'center',
    gap: 4,
  },
  primary: {
    background: '#6644ff',
    border: 0,
    color: '#fff',
    padding: '4px 12px',
    borderRadius: 3,
    cursor: 'pointer',
    fontSize: 11,
    display: 'inline-flex',
    alignItems: 'center',
    gap: 4,
    fontWeight: 500,
  },
}
