import { useState } from 'react'
import { Code } from 'lucide-react'
import { useSceneStore } from '../store/scene'

export function HtmlImporter() {
  const addHtmlLayer = useSceneStore((s) => s.addHtmlLayer)
  const [html, setHtml] = useState(
    `<div class="bg-gray-900 text-white rounded-2xl p-8 shadow-2xl">
  <h1 class="text-4xl font-bold text-violet-400">Acme</h1>
  <p class="mt-2 text-gray-400">Paste your Tailwind / HTML here.</p>
</div>`,
  )
  const [name, setName] = useState('HTML Card')
  const [busy, setBusy] = useState(false)

  const onAdd = async () => {
    if (!html.trim()) return
    setBusy(true)
    try {
      await addHtmlLayer(html, name)
    } finally {
      setBusy(false)
    }
  }

  return (
    <div style={styles.root}>
      <div style={styles.header}>
        <Code size={11} /> <span>HTML → LAYER</span>
      </div>
      <input
        style={styles.nameInput}
        placeholder="Layer name"
        value={name}
        onChange={(e) => setName(e.target.value)}
      />
      <textarea
        style={styles.textarea}
        placeholder="Paste raw HTML (Tailwind classes OK). Tailwind, Google Fonts, Lucide, Animate.css, Font Awesome are auto-injected."
        value={html}
        onChange={(e) => setHtml(e.target.value)}
        spellCheck={false}
      />
      <button style={styles.btn} disabled={busy || !html.trim()} onClick={onAdd}>
        {busy ? 'Adding…' : 'Add as HTML layer'}
      </button>
    </div>
  )
}

const styles: Record<string, React.CSSProperties> = {
  root: {
    display: 'flex',
    flexDirection: 'column',
    height: '100%',
    background: '#15151c',
    borderRight: '1px solid #2a2a36',
    padding: 8,
    gap: 6,
    overflow: 'hidden',
  },
  header: {
    fontSize: 10,
    fontWeight: 600,
    letterSpacing: 1,
    color: '#888',
    display: 'flex',
    alignItems: 'center',
    gap: 6,
    padding: '0 4px',
  },
  nameInput: {
    background: '#1f1f29',
    border: '1px solid #2a2a36',
    color: '#fff',
    padding: '4px 6px',
    borderRadius: 3,
    fontSize: 11,
    outline: 'none',
  },
  textarea: {
    flex: 1,
    minHeight: 80,
    background: '#1f1f29',
    border: '1px solid #2a2a36',
    color: '#cfcfdc',
    padding: '6px 8px',
    borderRadius: 3,
    fontSize: 11,
    fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
    outline: 'none',
    resize: 'none',
  },
  btn: {
    background: '#6644ff',
    border: 0,
    color: '#fff',
    padding: '6px 10px',
    borderRadius: 3,
    cursor: 'pointer',
    fontSize: 12,
    fontWeight: 500,
  },
}
