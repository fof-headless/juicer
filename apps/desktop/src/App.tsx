import { useEffect } from 'react'
import { Panel, PanelGroup, PanelResizeHandle } from 'react-resizable-panels'
import { Toolbar } from './components/Toolbar'
import { Sidebar } from './components/Sidebar'
import { Properties } from './components/Properties'
import { HtmlImporter } from './components/HtmlImporter'
import { Viewport } from './components/Viewport'
import { useSceneStore } from './store/scene'

export function App() {
  const refresh = useSceneStore((s) => s.refresh)

  useEffect(() => {
    refresh()
  }, [refresh])

  return (
    <div style={app.root}>
      <Toolbar />
      <div style={app.body}>
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
