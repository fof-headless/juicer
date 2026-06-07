import { useSceneStore } from '../store/scene'
import {
  Box, Circle, Type, Image, Code, Layers, Eye, EyeOff, Trash2,
  Plus, ChevronRight, Film,
} from 'lucide-react'

const TYPE_ICONS: Record<string, React.ReactNode> = {
  MESH:    <Box size={11} />,
  FONT:    <Type size={11} />,
  LIGHT:   <span style={{fontSize:10}}>☀</span>,
  CAMERA:  <span style={{fontSize:10}}>🎥</span>,
}

export function Sidebar() {
  const objects = useSceneStore((s) => s.objects)
  const selected = useSceneStore((s) => s.selectedName)
  const selectObject = useSceneStore((s) => s.selectObject)
  const cmd = useSceneStore((s) => s.cmd)
  const refreshScene = useSceneStore((s) => s.refreshScene)

  const addBox = async () => {
    await cmd('add_object', { kind: 'box', name: `Box_${Date.now()}`, location: [0, 0, 0], color: '#4488ff' })
    refreshScene()
  }

  const addSphere = async () => {
    await cmd('add_object', { kind: 'sphere', name: `Sphere_${Date.now()}`, location: [0, 0, 0], color: '#ff4488' })
    refreshScene()
  }

  const addText = async () => {
    await cmd('add_object', { kind: 'text', name: `Text_${Date.now()}`, location: [0, 0, 0], text: 'Your Brand', color: '#ffffff' })
    refreshScene()
  }

  const addPlane = async () => {
    await cmd('add_object', { kind: 'plane', name: `Plane_${Date.now()}`, location: [0, 0, 0], color: '#1a1a3e', width: 3, height: 2 })
    refreshScene()
  }

  const removeObject = async (name: string) => {
    await cmd('remove_object', { name })
    refreshScene()
  }

  const toggleVisible = async (name: string, visible: boolean) => {
    await cmd('update_object', { name, visible: !visible })
    refreshScene()
  }

  return (
    <div style={styles.sidebar}>
      {/* Scene outliner */}
      <div style={styles.section}>
        <div style={styles.sectionHeader}>
          <span>Scene</span>
          <span style={styles.badge}>{objects.length}</span>
        </div>
        <div style={styles.objectList}>
          {objects.length === 0 && (
            <div style={styles.empty}>Connect Blender to see objects</div>
          )}
          {objects.map((obj) => (
            <div
              key={obj.name}
              style={{
                ...styles.objectRow,
                ...(selected === obj.name ? styles.objectRowSelected : {}),
              }}
              onClick={() => selectObject(obj.name)}
            >
              <span style={styles.objectIcon}>
                {TYPE_ICONS[obj.type] ?? <Box size={11} />}
              </span>
              <span style={styles.objectName}>{obj.name}</span>
              <div style={styles.objectActions}>
                <button
                  style={styles.iconBtn}
                  onClick={(e) => { e.stopPropagation(); toggleVisible(obj.name, obj.visible) }}
                >
                  {obj.visible ? <Eye size={11} /> : <EyeOff size={11} />}
                </button>
                <button
                  style={{ ...styles.iconBtn, ...styles.deleteBtn }}
                  onClick={(e) => { e.stopPropagation(); removeObject(obj.name) }}
                >
                  <Trash2 size={11} />
                </button>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Quick add */}
      <div style={styles.section}>
        <div style={styles.sectionHeader}>Add</div>
        <div style={styles.quickAdd}>
          <button style={styles.qaBtn} onClick={addBox}>
            <Box size={13} /> Box
          </button>
          <button style={styles.qaBtn} onClick={addSphere}>
            <Circle size={13} /> Sphere
          </button>
          <button style={styles.qaBtn} onClick={addText}>
            <Type size={13} /> Text
          </button>
          <button style={styles.qaBtn} onClick={addPlane}>
            <Image size={13} /> Plane
          </button>
        </div>
      </div>
    </div>
  )
}

const styles: Record<string, React.CSSProperties> = {
  sidebar: {
    display: 'flex',
    flexDirection: 'column',
    height: '100%',
    background: '#111116',
    color: '#c8c8d4',
    fontSize: 12,
    fontFamily: "'Inter', 'Segoe UI', sans-serif",
    borderRight: '1px solid #1e1e28',
  },
  section: {
    borderBottom: '1px solid #1a1a22',
  },
  sectionHeader: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    padding: '7px 12px',
    fontSize: 10,
    fontWeight: 700,
    textTransform: 'uppercase',
    letterSpacing: '0.09em',
    color: '#555570',
    background: '#0f0f14',
  },
  badge: {
    background: '#1e1e2a',
    borderRadius: 10,
    padding: '1px 5px',
    fontSize: 10,
    color: '#666688',
  },
  objectList: {
    maxHeight: 300,
    overflowY: 'auto',
    padding: '3px 0',
  },
  empty: {
    padding: '16px 12px',
    color: '#333344',
    textAlign: 'center',
    fontSize: 11,
    lineHeight: 1.5,
  },
  objectRow: {
    display: 'flex',
    alignItems: 'center',
    gap: 6,
    padding: '4px 8px 4px 12px',
    cursor: 'pointer',
    borderLeft: '2px solid transparent',
    transition: 'background 0.1s',
  },
  objectRowSelected: {
    background: '#1a1a2e',
    borderLeftColor: '#6644ff',
  },
  objectIcon: { color: '#666688', flexShrink: 0, display: 'flex' },
  objectName: { flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' },
  objectActions: { display: 'flex', gap: 2, opacity: 0 },
  iconBtn: {
    background: 'none',
    border: 'none',
    color: '#666688',
    cursor: 'pointer',
    padding: 2,
    borderRadius: 3,
    display: 'flex',
    alignItems: 'center',
  },
  deleteBtn: {},
  quickAdd: {
    display: 'grid',
    gridTemplateColumns: '1fr 1fr',
    gap: 5,
    padding: 10,
  },
  qaBtn: {
    display: 'flex',
    alignItems: 'center',
    gap: 5,
    padding: '7px 8px',
    background: '#1a1a22',
    border: '1px solid #222230',
    borderRadius: 5,
    color: '#b0b0c8',
    fontSize: 12,
    cursor: 'pointer',
    transition: 'all 0.12s',
  },
}
