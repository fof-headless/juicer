import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useSceneStore } from '../store/scene'

const DEFAULT_BLENDER = '/Applications/Blender.app/Contents/MacOS/Blender'
const BRIDGE_SCRIPT_HINT = 'apps/blender-bridge/juicer_bridge.py'

export function BlenderSetup() {
  const connected = useSceneStore((s) => s.blenderConnected)
  const refresh = useSceneStore((s) => s.refreshScene)
  const [blenderPath, setBlenderPath] = useState(DEFAULT_BLENDER)
  const [bridgePath, setBridgePath] = useState('')
  const [status, setStatus] = useState<'idle' | 'connecting' | 'error'>('idle')
  const [errorMsg, setErrorMsg] = useState('')

  if (connected) return null

  const connect = async () => {
    setStatus('connecting')
    setErrorMsg('')
    try {
      await invoke('start_blender', {
        blenderPath,
        bridgeScript: bridgePath,
      })
      await refresh()
      setStatus('idle')
    } catch (e: any) {
      setStatus('error')
      setErrorMsg(e?.toString() ?? 'Unknown error')
    }
  }

  return (
    <div style={s.overlay}>
      <div style={s.card}>
        <div style={s.logo}>⚡ Juicer</div>
        <div style={s.title}>Connect Blender</div>
        <div style={s.sub}>Juicer uses Blender's Eevee GPU renderer under the hood. Point it at your Blender installation to start.</div>

        <div style={s.field}>
          <label style={s.label}>Blender executable</label>
          <input
            style={s.input}
            value={blenderPath}
            onChange={(e) => setBlenderPath(e.target.value)}
            placeholder="/Applications/Blender.app/Contents/MacOS/Blender"
          />
          <div style={s.hint}>Default Mac path shown. Adjust if you installed elsewhere.</div>
        </div>

        <div style={s.field}>
          <label style={s.label}>Bridge script (juicer_bridge.py)</label>
          <input
            style={s.input}
            value={bridgePath}
            onChange={(e) => setBridgePath(e.target.value)}
            placeholder="/path/to/juicer/apps/blender-bridge/juicer_bridge.py"
          />
          <div style={s.hint}>The Python file that ships with Juicer ({BRIDGE_SCRIPT_HINT})</div>
        </div>

        <button
          style={{ ...s.btn, ...(status === 'connecting' ? s.btnConnecting : {}) }}
          onClick={connect}
          disabled={status === 'connecting' || !blenderPath || !bridgePath}
        >
          {status === 'connecting' ? 'Starting Blender...' : 'Connect'}
        </button>

        {status === 'error' && (
          <div style={s.error}>{errorMsg}</div>
        )}

        <div style={s.prereqs}>
          <strong>Requirements:</strong>
          <ul>
            <li>Blender 4.0+ installed</li>
            <li>Xcode CLI tools (for the HTML capture helper)</li>
            <li>Rust + Tauri CLI for building this app</li>
          </ul>
          <div style={s.installCmd}>brew install blender (or download from blender.org)</div>
        </div>
      </div>
    </div>
  )
}

const s: Record<string, React.CSSProperties> = {
  overlay: {
    position: 'fixed',
    inset: 0,
    background: 'rgba(8, 8, 14, 0.92)',
    backdropFilter: 'blur(12px)',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    zIndex: 100,
  },
  card: {
    background: '#111118',
    border: '1px solid #222232',
    borderRadius: 14,
    padding: '32px 36px',
    width: 480,
    display: 'flex',
    flexDirection: 'column',
    gap: 18,
    boxShadow: '0 32px 80px rgba(0,0,0,0.6)',
  },
  logo: { fontSize: 28, letterSpacing: '-0.02em' },
  title: { fontSize: 20, fontWeight: 700, color: '#e8e8f4', fontFamily: "'Inter', sans-serif" },
  sub: { color: '#666688', fontSize: 13, lineHeight: 1.6, fontFamily: "'Inter', sans-serif" },
  field: { display: 'flex', flexDirection: 'column', gap: 5 },
  label: { fontSize: 11, fontWeight: 600, textTransform: 'uppercase', letterSpacing: '0.07em', color: '#555570', fontFamily: "'Inter', sans-serif" },
  input: {
    background: '#1a1a24',
    border: '1px solid #2a2a3a',
    borderRadius: 6,
    color: '#c8c8d4',
    padding: '8px 10px',
    fontSize: 12,
    fontFamily: "'JetBrains Mono', monospace",
    outline: 'none',
    width: '100%',
  },
  hint: { color: '#444458', fontSize: 10, fontFamily: "'Inter', sans-serif" },
  btn: {
    padding: '10px',
    background: '#6644ff',
    border: 'none',
    borderRadius: 8,
    color: 'white',
    fontSize: 13,
    fontWeight: 700,
    cursor: 'pointer',
    fontFamily: "'Inter', sans-serif",
  },
  btnConnecting: { background: '#2a2a3a', cursor: 'wait' },
  error: { color: '#cc4455', fontSize: 12, fontFamily: "'Inter', sans-serif", lineHeight: 1.5 },
  prereqs: {
    background: '#0d0d14',
    border: '1px solid #1a1a24',
    borderRadius: 6,
    padding: '12px 14px',
    color: '#555570',
    fontSize: 11,
    fontFamily: "'Inter', sans-serif",
    lineHeight: 1.7,
  },
  installCmd: {
    marginTop: 8,
    fontFamily: "'JetBrains Mono', monospace",
    color: '#444460',
    fontSize: 11,
  },
}
