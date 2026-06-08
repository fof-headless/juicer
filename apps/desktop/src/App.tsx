import { useEffect } from 'react'
import { Panel, PanelGroup, PanelResizeHandle } from 'react-resizable-panels'
import { Toolbar } from './components/Toolbar'
import { Sidebar } from './components/Sidebar'
import { Properties } from './components/Properties'
import { HtmlImporter } from './components/HtmlImporter'
import { Viewport } from './components/Viewport'
import { Timeline } from './components/Timeline'
import { useSceneStore } from './store/scene'

export function App() {
  const refresh = useSceneStore((s) => s.refresh)
  const scene = useSceneStore((s) => s.scene)
  const frame = useSceneStore((s) => s.frame)
  const renderPreview = useSceneStore((s) => s.renderPreview)

  useEffect(() => {
    refresh()
  }, [refresh])

  // Live preview: re-render shortly after any scene edit or frame change, so
  // the viewport reflects edits without manually clicking. Skipped during
  // playback (the Toolbar loop drives its own frames).
  useEffect(() => {
    if (!scene || scene.elements.length === 0) return
    const t = setTimeout(() => {
      if (!(useSceneStore.getState() as any).__playing) renderPreview(frame)
    }, 140)
    return () => clearTimeout(t)
  }, [scene, frame, renderPreview])

  return (
    <div style={app.root}>
      <Toolbar />
      <div style={app.body}>
        <PanelGroup direction="vertical">
          <Panel defaultSize={74} minSize={40}>
            <PanelGroup direction="horizontal">
              <Panel defaultSize={20} minSize={14} maxSize={30}>
                <PanelGroup direction="vertical">
                  <Panel defaultSize={50} minSize={20}><Sidebar /></Panel>
                  <PanelResizeHandle style={handle.h} />
                  <Panel defaultSize={50} minSize={20}><HtmlImporter /></Panel>
                </PanelGroup>
              </Panel>

              <PanelResizeHandle style={handle.v} />

              <Panel defaultSize={58} minSize={30}><Viewport /></Panel>

              <PanelResizeHandle style={handle.v} />

              <Panel defaultSize={22} minSize={14} maxSize={32}><Properties /></Panel>
            </PanelGroup>
          </Panel>

          <PanelResizeHandle style={handle.h} />

          <Panel defaultSize={26} minSize={12} maxSize={45}><Timeline /></Panel>
        </PanelGroup>
      </div>
    </div>
  )
}

const app: Record<string, React.CSSProperties> = {
  root: { display: 'flex', flexDirection: 'column', width: '100vw', height: '100vh', overflow: 'hidden', background: '#0d0d12' },
  body: { flex: 1, minHeight: 0, overflow: 'hidden' },
}

const handle: Record<string, React.CSSProperties> = {
  v: { width: 4, background: '#1a1a22', cursor: 'col-resize', flexShrink: 0 },
  h: { height: 4, background: '#1a1a22', cursor: 'row-resize', flexShrink: 0 },
}
