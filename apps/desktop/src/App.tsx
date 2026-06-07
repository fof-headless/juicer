import { useEffect } from 'react'
import { Panel, PanelGroup, PanelResizeHandle } from 'react-resizable-panels'
import { Toolbar } from './components/Toolbar'
import { Sidebar } from './components/Sidebar'
import { Properties } from './components/Properties'
import { HtmlImporter } from './components/HtmlImporter'
import { BlenderSetup } from './components/BlenderSetup'
import { useSceneStore } from './store/scene'

// The Blender viewport is rendered by Blender itself (headless Eevee).
// Juicer's UI wraps around it — outliner, properties, HTML importer.
// When Blender is running in a window-mode build, we can embed its window.
// In headless mode, the viewport shows render previews as images.
function ViewportPlaceholder() {
  const connected = useSceneStore((s) => s.blenderConnected)
  const objects = useSceneStore((s) => s.objects)
  const cmd = useSceneStore((s) => s.cmd)
  const refresh = useSceneStore((s) => s.refreshScene)
  const [previewUrl, setPreviewUrl] = useState<string | null>(null)

  const renderPreview = async () => {
    if (!connected) return
    const out = `/tmp/juicer_preview_${Date.now()}.png`
    try {
      await cmd('render_frame', { frame: 1, output_path: out })
      // Load as data URL
      setPreviewUrl(`file://${out}`)
    } catch {}
  }

  if (!connected) {
    return (
      <div style={vp.empty}>
        <div style={vp.emptyIcon}>⚡</div>
        <div style={vp.emptyTitle}>Connect Blender to start</div>
        <div style={vp.emptyHint}>Eevee renders will appear here</div>
      </div>
    )
  }

  return (
    <div style={vp.container}>
      <div style={vp.toolbar}>
        <button style={vp.previewBtn} onClick={renderPreview}>
          Preview Frame (Eevee)
        </button>
        <span style={vp.info}>{objects.length} objects in scene</span>
      </div>
      {previewUrl ? (
        <img src={previewUrl} style={vp.preview} alt="Blender render preview" />
      ) : (
        <div style={vp.noPreview}>
          Hit "Preview Frame" to render via Eevee.<br />
          Full animation: use the Render button in the toolbar.
        </div>
      )}
    </div>
  )
}

// useState needed above — import it
import { useState } from 'react'

export function App() {
  const refresh = useSceneStore((s) => s.refreshScene)

  // Poll Blender connection status every 3s
  useEffect(() => {
    const id = setInterval(refresh, 3000)
    return () => clearInterval(id)
  }, [refresh])

  return (
    <div style={app.root}>
      <BlenderSetup />
      <Toolbar />
      <div style={app.body}>
        <PanelGroup direction="horizontal">

          {/* Left: outliner + HTML importer */}
          <Panel defaultSize={20} minSize={14} maxSize={30}>
            <PanelGroup direction="vertical">
              <Panel defaultSize={50} minSize={20}>
                <Sidebar />
              </Panel>
              <PanelResizeHandle style={handle.h} />
              <Panel defaultSize={50} minSize={20}>
                <HtmlImporter />
              </Panel>
            </PanelGroup>
          </Panel>

          <PanelResizeHandle style={handle.v} />

          {/* Center: viewport */}
          <Panel defaultSize={58} minSize={30}>
            <ViewportPlaceholder />
          </Panel>

          <PanelResizeHandle style={handle.v} />

          {/* Right: properties */}
          <Panel defaultSize={22} minSize={14} maxSize={32}>
            <Properties />
          </Panel>

        </PanelGroup>
      </div>
    </div>
  )
}

const app: Record<string, React.CSSProperties> = {
  root: {
    display: 'flex',
    flexDirection: 'column',
    width: '100vw',
    height: '100vh',
    overflow: 'hidden',
    background: '#0d0d12',
  },
  body: {
    flex: 1,
    minHeight: 0,
    overflow: 'hidden',
  },
}

const handle: Record<string, React.CSSProperties> = {
  v: { width: 4, background: '#1a1a22', cursor: 'col-resize', flexShrink: 0 },
  h: { height: 4, background: '#1a1a22', cursor: 'row-resize', flexShrink: 0 },
}

const vp: Record<string, React.CSSProperties> = {
  container: {
    display: 'flex',
    flexDirection: 'column',
    height: '100%',
    background: '#0a0a10',
  },
  toolbar: {
    display: 'flex',
    alignItems: 'center',
    gap: 10,
    padding: '6px 12px',
    background: '#0f0f16',
    borderBottom: '1px solid #1a1a22',
  },
  previewBtn: {
    padding: '4px 12px',
    background: '#1e1e2a',
    border: '1px solid #2a2a3a',
    borderRadius: 5,
    color: '#9988cc',
    fontSize: 11,
    cursor: 'pointer',
    fontWeight: 600,
  },
  info: { fontSize: 11, color: '#333348' },
  preview: {
    flex: 1,
    objectFit: 'contain',
    width: '100%',
    height: '100%',
    display: 'block',
  },
  noPreview: {
    flex: 1,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    color: '#2a2a3a',
    fontSize: 13,
    textAlign: 'center',
    lineHeight: 1.8,
    fontFamily: "'Inter', sans-serif",
  },
  empty: {
    height: '100%',
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    justifyContent: 'center',
    gap: 10,
  },
  emptyIcon: { fontSize: 48 },
  emptyTitle: { fontSize: 15, color: '#333348', fontFamily: "'Inter', sans-serif", fontWeight: 600 },
  emptyHint: { fontSize: 12, color: '#222232', fontFamily: "'Inter', sans-serif" },
}
