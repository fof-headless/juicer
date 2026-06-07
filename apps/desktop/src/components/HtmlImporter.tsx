import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useSceneStore } from '../store/scene'
import { Code, Upload } from 'lucide-react'

export function HtmlImporter() {
  const cmd = useSceneStore((s) => s.cmd)
  const refresh = useSceneStore((s) => s.refreshScene)
  const [html, setHtml] = useState('')
  const [name, setName] = useState('HTMLPlane')
  const [width, setWidth] = useState(1200)
  const [height, setHeight] = useState(750)
  const [status, setStatus] = useState<'idle' | 'capturing' | 'done' | 'error'>('idle')
  const [imagePath, setImagePath] = useState('')

  const capture = async () => {
    if (!html.trim()) return
    setStatus('capturing')
    const tmpPath = `/tmp/juicer_html_${Date.now()}.html`
    const outPath = `/tmp/juicer_capture_${Date.now()}.png`

    try {
      // Write HTML to temp file via Tauri FS, then capture
      // (Tauri FS plugin would be used here; for now we use the invoke directly)
      const path = await invoke<string>('capture_html', {
        html: `<!doctype html><html><head><meta charset="utf-8"><style>*{box-sizing:border-box;margin:0;padding:0}html,body{width:${width}px;height:${height}px;overflow:hidden;background:#0d0d18}</style></head><body>${html}</body></html>`,
        width,
        height,
        outputPath: outPath,
      })
      setImagePath(path)
      setStatus('done')

      // Add as a plane with the captured image
      await cmd('add_object', {
        kind: 'plane',
        name,
        location: [0, 0, 0],
        rotation: [1.5708, 0, 0],
        scale: [1, 1, 1],
        image_path: path,
        width: 4,
        height: (height / width) * 4,
      })
      refresh()
    } catch (e: any) {
      console.error(e)
      setStatus('error')
    }
  }

  return (
    <div style={s.panel}>
      <div style={s.header}>HTML → 3D Plane</div>

      <div style={s.body}>
        <label style={s.label}>Element name</label>
        <input
          style={s.input}
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="MyComponent"
        />

        <div style={s.row}>
          <div style={s.halfField}>
            <label style={s.label}>Width px</label>
            <input type="number" style={s.input} value={width} onChange={(e) => setWidth(+e.target.value)} />
          </div>
          <div style={s.halfField}>
            <label style={s.label}>Height px</label>
            <input type="number" style={s.input} value={height} onChange={(e) => setHeight(+e.target.value)} />
          </div>
        </div>

        <label style={s.label}>HTML / component output</label>
        <textarea
          style={s.textarea}
          value={html}
          onChange={(e) => setHtml(e.target.value)}
          rows={8}
          placeholder={`<div style="background:#1a1a3e;color:white;padding:32px;font-family:sans-serif;border-radius:16px;border:1px solid #6644ff">\n  <h1>Your Brand</h1>\n  <p>Paste your React/HTML output here</p>\n</div>`}
        />

        <button
          style={{ ...s.btn, ...(status === 'capturing' ? s.btnDisabled : {}) }}
          onClick={capture}
          disabled={!html.trim() || status === 'capturing'}
        >
          <Upload size={13} />
          {status === 'capturing' ? 'Capturing via WebKit...' : 'Capture & Add to Scene'}
        </button>

        {status === 'done' && (
          <div style={s.success}>✓ Added as 3D plane: {imagePath.split('/').pop()}</div>
        )}
        {status === 'error' && (
          <div style={s.error}>Build the html-capture helper first (see README).</div>
        )}

        <div style={s.hint}>
          Uses macOS native WebKit — full CSS3, fonts, gradients. The capture is pixel-perfect, then applied as a GPU texture on a Blender plane.
        </div>
      </div>
    </div>
  )
}

const s: Record<string, React.CSSProperties> = {
  panel: {
    display: 'flex',
    flexDirection: 'column',
    height: '100%',
    background: '#111116',
    color: '#c8c8d4',
    fontSize: 12,
    fontFamily: "'Inter', 'Segoe UI', sans-serif",
  },
  header: {
    padding: '7px 12px',
    fontSize: 10,
    fontWeight: 700,
    textTransform: 'uppercase',
    letterSpacing: '0.09em',
    color: '#555570',
    background: '#0f0f14',
    borderBottom: '1px solid #1a1a22',
  },
  body: {
    padding: '10px 12px',
    display: 'flex',
    flexDirection: 'column',
    gap: 7,
    flex: 1,
    overflowY: 'auto',
  },
  label: { color: '#555570', fontSize: 10, fontWeight: 600, textTransform: 'uppercase', letterSpacing: '0.07em' },
  input: {
    background: '#1a1a22',
    border: '1px solid #222232',
    borderRadius: 4,
    color: '#c8c8d4',
    padding: '5px 8px',
    fontSize: 12,
    outline: 'none',
    width: '100%',
  },
  row: { display: 'flex', gap: 8 },
  halfField: { flex: 1, display: 'flex', flexDirection: 'column', gap: 4 },
  textarea: {
    background: '#13131a',
    border: '1px solid #222232',
    borderRadius: 5,
    color: '#c8c8d4',
    padding: '8px',
    fontSize: 11,
    fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
    resize: 'vertical',
    outline: 'none',
    width: '100%',
  },
  btn: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    gap: 6,
    padding: '8px',
    background: '#5533bb',
    border: 'none',
    borderRadius: 6,
    color: 'white',
    fontSize: 12,
    fontWeight: 600,
    cursor: 'pointer',
  },
  btnDisabled: { background: '#2a2a3a', color: '#555566', cursor: 'not-allowed' },
  success: { color: '#44cc77', fontSize: 11 },
  error: { color: '#cc4455', fontSize: 11, lineHeight: 1.5 },
  hint: { color: '#333348', fontSize: 10, lineHeight: 1.6, borderTop: '1px solid #1a1a22', paddingTop: 8, marginTop: 4 },
}
