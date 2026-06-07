import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useSceneStore } from '../store/scene'
import { Upload } from 'lucide-react'

export function HtmlImporter() {
  const addElement = useSceneStore((s) => s.addElement)
  const refresh = useSceneStore((s) => s.refresh)
  const [html, setHtml] = useState('')
  const [name, setName] = useState('HTMLPlane')
  const [width, setWidth] = useState(1200)
  const [height, setHeight] = useState(750)
  const [status, setStatus] = useState<'idle' | 'busy' | 'done' | 'error'>('idle')
  const [msg, setMsg] = useState('')

  const capture = async () => {
    if (!html.trim()) return
    setStatus('busy')
    setMsg('')
    const out = `/tmp/juicer_capture_${Date.now()}.png`
    const doc = `<!doctype html><html><head><meta charset="utf-8"><style>*{box-sizing:border-box;margin:0;padding:0}html,body{width:${width}px;height:${height}px;overflow:hidden;background:#0d0d18}</style></head><body>${html}</body></html>`
    try {
      const path = await invoke<string>('capture_html', { html: doc, width, height, outputPath: out })
      const aspect = height / width
      await addElement('plane', name, { image_path: path, width: 4, height: 4 * aspect, unlit: true })
      await refresh()
      setStatus('done')
      setMsg(`Added "${name}" with captured texture`)
    } catch (e: any) {
      setStatus('error')
      setMsg(e?.toString?.() ?? 'capture failed — build juicer-html-capture (see README)')
    }
  }

  return (
    <div style={s.panel}>
      <div style={s.header}>HTML → 3D Plane</div>
      <div style={s.body}>
        <label style={s.label}>Element name</label>
        <input style={s.input} value={name} onChange={(e) => setName(e.target.value)} />

        <div style={{ display: 'flex', gap: 8 }}>
          <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 4 }}>
            <label style={s.label}>Width px</label>
            <input type="number" style={s.input} value={width} onChange={(e) => setWidth(+e.target.value)} />
          </div>
          <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 4 }}>
            <label style={s.label}>Height px</label>
            <input type="number" style={s.input} value={height} onChange={(e) => setHeight(+e.target.value)} />
          </div>
        </div>

        <label style={s.label}>HTML / component output</label>
        <textarea
          style={s.textarea}
          rows={8}
          value={html}
          onChange={(e) => setHtml(e.target.value)}
          placeholder={`<div style="background:#1a1a3e;color:#fff;padding:32px;font-family:sans-serif;border-radius:16px;border:1px solid #6644ff">\n  <h1>Your Brand</h1>\n  <p>Paste your React/HTML output</p>\n</div>`}
        />

        <button style={{ ...s.btn, ...(status === 'busy' ? s.btnBusy : {}) }} onClick={capture} disabled={!html.trim() || status === 'busy'}>
          <Upload size={13} />
          {status === 'busy' ? 'Capturing via WebKit…' : 'Capture & Add'}
        </button>

        {status === 'done' && <div style={{ color: '#44cc77', fontSize: 11 }}>✓ {msg}</div>}
        {status === 'error' && <div style={{ color: '#cc4455', fontSize: 11, lineHeight: 1.5 }}>{msg}</div>}

        <div style={s.hint}>Native macOS WebKit renders your HTML pixel-perfect, then it's applied as a GPU texture on a plane. Full CSS3, fonts, gradients.</div>
      </div>
    </div>
  )
}

const s: Record<string, React.CSSProperties> = {
  panel: { display: 'flex', flexDirection: 'column', height: '100%', background: '#111116', color: '#c8c8d4', fontSize: 12, fontFamily: "'Inter', sans-serif" },
  header: { padding: '7px 12px', fontSize: 10, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.09em', color: '#555570', background: '#0f0f14', borderBottom: '1px solid #1a1a22' },
  body: { padding: '10px 12px', display: 'flex', flexDirection: 'column', gap: 7, flex: 1, overflowY: 'auto' },
  label: { color: '#555570', fontSize: 10, fontWeight: 600, textTransform: 'uppercase', letterSpacing: '0.07em' },
  input: { background: '#1a1a22', border: '1px solid #222232', borderRadius: 4, color: '#c8c8d4', padding: '5px 8px', fontSize: 12, outline: 'none', width: '100%' },
  textarea: { background: '#13131a', border: '1px solid #222232', borderRadius: 5, color: '#c8c8d4', padding: '8px', fontSize: 11, fontFamily: "'JetBrains Mono', monospace", resize: 'vertical', outline: 'none', width: '100%' },
  btn: { display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 6, padding: '8px', background: '#5533bb', border: 'none', borderRadius: 6, color: 'white', fontSize: 12, fontWeight: 600, cursor: 'pointer' },
  btnBusy: { background: '#2a2a3a', color: '#555566', cursor: 'wait' },
  hint: { color: '#333348', fontSize: 10, lineHeight: 1.6, borderTop: '1px solid #1a1a22', paddingTop: 8, marginTop: 4 },
}
