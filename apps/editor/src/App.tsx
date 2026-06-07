import { PanelGroup, Panel, PanelResizeHandle } from 'react-resizable-panels'
import { Viewport } from './viewport/Viewport'
import { Outliner } from './outliner/Outliner'
import { Properties } from './properties/Properties'
import { HtmlImporter } from './importer/HtmlImporter'
import { Recorder } from './recorder/Recorder'
import { Toolbar } from './Toolbar'
import { McpBridge } from './mcp/McpBridge'
import { useSceneStore } from './store/scene'
import './App.css'

export function App() {
  const wsConnected = useSceneStore((s) => s.wsConnected)

  return (
    <div className="app">
      {/* MCP WebSocket bridge (invisible) */}
      <McpBridge />

      {/* Top toolbar */}
      <Toolbar />

      {/* Main layout */}
      <div className="app-body">
        <PanelGroup direction="horizontal">
          {/* Left sidebar */}
          <Panel defaultSize={18} minSize={12} maxSize={28}>
            <PanelGroup direction="vertical">
              <Panel defaultSize={45} minSize={20}>
                <Outliner />
              </Panel>
              <PanelResizeHandle className="resize-handle-h" />
              <Panel defaultSize={55} minSize={20}>
                <HtmlImporter />
              </Panel>
            </PanelGroup>
          </Panel>

          <PanelResizeHandle className="resize-handle-v" />

          {/* Center: 3D viewport */}
          <Panel defaultSize={60} minSize={30}>
            <Viewport />
          </Panel>

          <PanelResizeHandle className="resize-handle-v" />

          {/* Right sidebar: Properties + Recorder */}
          <Panel defaultSize={22} minSize={14} maxSize={32}>
            <PanelGroup direction="vertical">
              <Panel defaultSize={75} minSize={30}>
                <Properties />
              </Panel>
              <PanelResizeHandle className="resize-handle-h" />
              <Panel defaultSize={25} minSize={15}>
                <Recorder />
              </Panel>
            </PanelGroup>
          </Panel>
        </PanelGroup>
      </div>

      {/* MCP status badge */}
      <div className={`mcp-badge ${wsConnected ? 'connected' : 'disconnected'}`}>
        <span className="mcp-dot" />
        {wsConnected ? 'Claude MCP Connected' : 'MCP Server Offline'}
      </div>

      {/* Theatre.js Studio mounts itself into the page automatically */}
    </div>
  )
}
