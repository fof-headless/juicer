import { useEffect, useState } from 'react'
import { useSceneStore } from '../store/scene'
import { Play, Pause, SkipBack, Video, Loader, Image, Save, Folder, FolderPlus } from 'lucide-react'

export function Toolbar() {
  const scene = useSceneStore((s) => s.scene)
  const project = useSceneStore((s) => s.project)
  const refreshProject = useSceneStore((s) => s.refreshProject)
  const saveProject = useSceneStore((s) => s.saveProject)
  const createProject = useSceneStore((s) => s.createProject)
  const frame = useSceneStore((s) => s.frame)
  const setFrame = useSceneStore((s) => s.setFrame)
  const renderPreview = useSceneStore((s) => s.renderPreview)
  const renderVideo = useSceneStore((s) => s.renderVideo)
  const isRendering = useSceneStore((s) => s.isRendering)

  useEffect(() => { refreshProject() }, [refreshProject])

  const [playing, setPlaying] = useState(false)
  const [saved, setSaved] = useState(false)
  const [newOpen, setNewOpen] = useState(false)
  const [newName, setNewName] = useState('Untitled')
  const [toast, setToast] = useState<{ kind: 'ok' | 'err'; msg: string } | null>(null)

  const fps = scene?.render.fps ?? 30
  const frameEnd = scene?.render.frame_end ?? 300
  const frameStart = scene?.render.frame_start ?? 1

  const flash = (kind: 'ok' | 'err', msg: string) => {
    setToast({ kind, msg })
    setTimeout(() => setToast(null), kind === 'err' ? 6000 : 3500)
  }

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
    ;(useSceneStore as any).setState({ __playing: true })
    const stop = () => { clearInterval(interval); ;(useSceneStore as any).setState({ __playing: false }) }
    ;(window as any).__juicerStop = stop
  }

  const stopPlay = () => {
    setPlaying(false)
    ;(window as any).__juicerStop?.()
  }

  const onPlayClick = () => { if (playing) stopPlay(); else togglePlay() }

  const rewind = async () => { stopPlay(); setFrame(frameStart); await renderPreview(frameStart) }

  const doRender = async () => {
    try {
      const path = await renderVideo()
      flash('ok', `Render complete → ${path}`)
    } catch (e: any) {
      flash('err', `Render failed: ${e?.toString?.() ?? e}. Make sure juicer-encoder is built (pnpm helpers).`)
    }
  }

  const doSave = async () => {
    try {
      await saveProject()
      setSaved(true)
      setTimeout(() => setSaved(false), 1500)
    } catch (e: any) {
      flash('err', `Save error: ${e?.toString?.() ?? e}`)
    }
  }

  const submitNewProject = async () => {
    const name = newName.trim()
    if (!name) return
    try {
      await createProject(name)
      setNewOpen(false)
      setNewName('Untitled')
      flash('ok', `Project "${name}" created`)
    } catch (e: any) {
      flash('err', `Could not create project: ${e?.toString?.() ?? e}`)
    }
  }

  const tc = `${String(Math.floor(frame / fps / 60)).padStart(2, '0')}:${String(Math.floor((frame / fps) % 60)).padStart(2, '0')} · f${frame}`

  return (
    <>
      <div style={s.bar}>
        <div style={s.brand}>
          <span style={s.logo}>⚡ Juicer</span>
          <span style={s.mode}>{(scene?.mode ?? 'lite').toUpperCase()}</span>
        </div>

        {project && (
          <div style={s.project} title={project.root}>
            <Folder size={11} />
            <span style={s.projectName}>{project.name}</span>
          </div>
        )}
        <button style={s.newBtn} onClick={() => { setNewName('Untitled'); setNewOpen(true) }} title="New project">
          <FolderPlus size={13} />
        </button>

        <div style={s.playback}>
          <button style={s.btn} onClick={rewind}><SkipBack size={13} /></button>
          <button style={{ ...s.btn, color: playing ? '#9977ff' : '#c8c8d4' }} onClick={onPlayClick}>
            {playing ? <Pause size={15} /> : <Play size={15} />}
          </button>
          <span style={s.tc}>{tc}</span>
        </div>

        <div style={s.right}>
          <button style={s.previewBtn} onClick={doSave}>
            <Save size={12} /> {saved ? 'Saved ✓' : 'Save'}
          </button>
          <button style={s.previewBtn} onClick={() => renderPreview()}>
            <Image size={12} /> Preview
          </button>
          <button style={s.renderBtn} onClick={doRender} disabled={isRendering}>
            {isRendering ? <Loader size={13} /> : <Video size={13} />}
            {isRendering ? 'Rendering…' : 'Render MP4'}
          </button>
        </div>
      </div>

      {toast && (
        <div style={{ ...s.toast, ...(toast.kind === 'err' ? s.toastErr : s.toastOk) }} onClick={() => setToast(null)}>
          {toast.msg}
        </div>
      )}

      {newOpen && (
        <div style={s.overlay} onMouseDown={() => setNewOpen(false)}>
          <div style={s.modal} onMouseDown={(e) => e.stopPropagation()}>
            <div style={s.modalTitle}>New Project</div>
            <div style={s.modalHint}>Creates ~/Movies/Juicer/&lt;name&gt;/ with assets &amp; renders folders.</div>
            <input
              style={s.modalInput}
              autoFocus
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              onKeyDown={(e) => { if (e.key === 'Enter') submitNewProject(); if (e.key === 'Escape') setNewOpen(false) }}
            />
            <div style={s.modalActions}>
              <button style={s.modalCancel} onClick={() => setNewOpen(false)}>Cancel</button>
              <button style={s.modalCreate} onClick={submitNewProject}>Create</button>
            </div>
          </div>
        </div>
      )}
    </>
  )
}

const s: Record<string, React.CSSProperties> = {
  bar: { display: 'flex', alignItems: 'center', height: 44, background: '#0a0a10', borderBottom: '1px solid #1a1a22', padding: '0 14px 0 80px', gap: 16, flexShrink: 0, ['WebkitAppRegion' as any]: 'drag' },
  brand: { display: 'flex', alignItems: 'baseline', gap: 8, ['WebkitAppRegion' as any]: 'no-drag' },
  logo: { fontWeight: 700, fontSize: 14, color: '#8866ff', letterSpacing: '-0.02em' },
  mode: { fontSize: 9, fontWeight: 700, color: '#44cc77', border: '1px solid rgba(68,204,119,0.3)', borderRadius: 4, padding: '1px 5px', letterSpacing: '0.08em' },
  project: { display: 'flex', alignItems: 'center', gap: 5, color: '#7777aa', fontSize: 11, fontWeight: 600, background: '#141420', border: '1px solid #1e1e2e', borderRadius: 6, padding: '3px 8px', ['WebkitAppRegion' as any]: 'no-drag', maxWidth: 160, overflow: 'hidden' },
  projectName: { whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' },
  newBtn: { display: 'flex', alignItems: 'center', justifyContent: 'center', background: '#141420', border: '1px solid #1e1e2e', borderRadius: 6, color: '#7777aa', cursor: 'pointer', padding: '4px 6px', ['WebkitAppRegion' as any]: 'no-drag' },
  playback: { display: 'flex', alignItems: 'center', gap: 5, background: '#141420', padding: '4px 8px', borderRadius: 8, border: '1px solid #1e1e2e', ['WebkitAppRegion' as any]: 'no-drag' },
  btn: { display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'none', border: 'none', color: '#7777aa', cursor: 'pointer', padding: 5, borderRadius: 4, ['WebkitAppRegion' as any]: 'no-drag' },
  tc: { fontFamily: 'monospace', fontSize: 11, color: '#555570', minWidth: 90 },
  right: { marginLeft: 'auto', display: 'flex', alignItems: 'center', gap: 8, ['WebkitAppRegion' as any]: 'no-drag' },
  previewBtn: { display: 'flex', alignItems: 'center', gap: 5, padding: '5px 11px', background: '#1e1e2a', border: '1px solid #2a2a3a', borderRadius: 6, color: '#9988cc', fontSize: 12, fontWeight: 600, cursor: 'pointer' },
  renderBtn: { display: 'flex', alignItems: 'center', gap: 5, padding: '5px 12px', background: '#aa2233', border: 'none', borderRadius: 6, color: 'white', fontSize: 12, fontWeight: 600, cursor: 'pointer' },

  toast: { position: 'fixed', top: 52, left: '50%', transform: 'translateX(-50%)', zIndex: 1000, padding: '9px 16px', borderRadius: 8, fontSize: 12, fontWeight: 600, cursor: 'pointer', maxWidth: '70vw', boxShadow: '0 6px 24px rgba(0,0,0,0.5)' },
  toastOk: { background: '#163a26', color: '#5fd99a', border: '1px solid #2a6a45' },
  toastErr: { background: '#3a1620', color: '#ff8095', border: '1px solid #6a2a3a' },

  overlay: { position: 'fixed', inset: 0, zIndex: 1001, background: 'rgba(0,0,0,0.55)', display: 'flex', alignItems: 'center', justifyContent: 'center' },
  modal: { width: 360, background: '#15151e', border: '1px solid #2a2a3a', borderRadius: 12, padding: 20, boxShadow: '0 20px 60px rgba(0,0,0,0.6)' },
  modalTitle: { fontSize: 15, fontWeight: 700, color: '#c8c8d8', marginBottom: 4 },
  modalHint: { fontSize: 11, color: '#555570', marginBottom: 14, lineHeight: 1.5 },
  modalInput: { width: '100%', background: '#0d0d14', border: '1px solid #2a2a3a', borderRadius: 6, color: '#e0e0ee', padding: '9px 11px', fontSize: 13, outline: 'none' },
  modalActions: { display: 'flex', justifyContent: 'flex-end', gap: 8, marginTop: 16 },
  modalCancel: { padding: '7px 14px', background: '#1e1e2a', border: '1px solid #2a2a3a', borderRadius: 6, color: '#9999b0', fontSize: 12, fontWeight: 600, cursor: 'pointer' },
  modalCreate: { padding: '7px 16px', background: '#5533bb', border: 'none', borderRadius: 6, color: 'white', fontSize: 12, fontWeight: 700, cursor: 'pointer' },
}
