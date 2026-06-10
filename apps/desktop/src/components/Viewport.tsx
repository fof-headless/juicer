import { useEffect, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useSceneStore } from '../store/scene'

/// The viewport is the same `renderer.html` the offscreen Swift helper loads —
/// just embedded as an iframe and driven by postMessage. What you see here is
/// pixel-identical to what `render_frame` will produce.
export function Viewport() {
  const iframeRef = useRef<HTMLIFrameElement | null>(null)
  const scene = useSceneStore((s) => s.scene)
  const frame = useSceneStore((s) => s.frame)
  const lastSent = useRef<string>('')

  // Push the current scene+state into the iframe whenever the scene or frame
  // changes. The iframe runtime listens for { kind: 'juicer-render', payload }.
  useEffect(() => {
    if (!scene) return
    const send = async () => {
      try {
        const payload = await invoke<unknown>('preview_state', { frame })
        const hash = JSON.stringify(payload)
        if (hash === lastSent.current) return
        lastSent.current = hash
        iframeRef.current?.contentWindow?.postMessage(
          { kind: 'juicer-render', payload },
          '*',
        )
      } catch (e) {
        console.error('viewport push', e)
      }
    }
    // Slight debounce so rapid scene edits don't pile up postMessages.
    const t = setTimeout(send, 30)
    return () => clearTimeout(t)
  }, [scene, frame])

  const aspect = scene ? scene.canvas.width / Math.max(1, scene.canvas.height) : 16 / 9

  return (
    <div style={styles.wrap}>
      <div style={{ ...styles.canvasBox, aspectRatio: `${aspect}` }}>
        <iframe
          ref={iframeRef}
          src="/renderer.html"
          title="Juicer viewport"
          style={styles.iframe}
        />
      </div>
      <div style={styles.footer}>
        <span>
          {scene
            ? `${scene.canvas.width}×${scene.canvas.height} • ${scene.canvas.fps} fps • ${scene.duration_frames} frames`
            : 'no scene'}
        </span>
        <span style={{ marginLeft: 'auto' }}>frame {frame}</span>
      </div>
    </div>
  )
}

const styles: Record<string, React.CSSProperties> = {
  wrap: {
    display: 'flex',
    flexDirection: 'column',
    height: '100%',
    width: '100%',
    background: '#1a1a22',
    overflow: 'hidden',
  },
  canvasBox: {
    flex: 1,
    minHeight: 0,
    margin: 16,
    background:
      'repeating-conic-gradient(#1f1f29 0% 25%, #181820 0% 50%) 50% / 24px 24px',
    boxShadow: '0 0 0 1px #2a2a36, 0 12px 48px -8px rgba(0,0,0,0.6)',
    borderRadius: 6,
    overflow: 'hidden',
    alignSelf: 'center',
    maxHeight: 'calc(100% - 64px)',
  },
  iframe: {
    width: '100%',
    height: '100%',
    border: 0,
    background: 'transparent',
    display: 'block',
  },
  footer: {
    display: 'flex',
    alignItems: 'center',
    padding: '6px 16px',
    fontSize: 11,
    color: '#9a9aa8',
    borderTop: '1px solid #2a2a36',
    background: '#15151c',
  },
}
