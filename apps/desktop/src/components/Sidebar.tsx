import { useSceneStore, ElementKind } from '../store/scene'
import { Box, Circle, Square, Eye, EyeOff, Trash2 } from 'lucide-react'

const KIND_ICON: Record<ElementKind, React.ReactNode> = {
  plane: <Square size={11} />,
  box: <Box size={11} />,
  sphere: <Circle size={11} />,
}

export function Sidebar() {
  const scene = useSceneStore((s) => s.scene)
  const selectedId = useSceneStore((s) => s.selectedId)
  const select = useSceneStore((s) => s.select)
  const addElement = useSceneStore((s) => s.addElement)
  const updateElement = useSceneStore((s) => s.updateElement)
  const removeElement = useSceneStore((s) => s.removeElement)

  const elements = scene?.elements ?? []

  return (
    <div style={s.sidebar}>
      <div style={s.section}>
        <div style={s.head}>
          <span>Scene</span>
          <span style={s.badge}>{elements.length}</span>
        </div>
        <div style={s.list}>
          {elements.length === 0 && <div style={s.empty}>Empty scene. Add elements below.</div>}
          {elements.map((el) => (
            <div
              key={el.id}
              style={{ ...s.row, ...(selectedId === el.id ? s.rowSel : {}) }}
              onClick={() => select(el.id)}
            >
              <span style={s.icon}>{KIND_ICON[el.kind]}</span>
              <span style={s.name}>{el.name}</span>
              <div style={s.actions}>
                <button style={s.iconBtn} onClick={(e) => { e.stopPropagation(); updateElement(el.id, { visible: !el.visible }) }}>
                  {el.visible ? <Eye size={11} /> : <EyeOff size={11} />}
                </button>
                <button style={s.iconBtn} onClick={(e) => { e.stopPropagation(); removeElement(el.id) }}>
                  <Trash2 size={11} />
                </button>
              </div>
            </div>
          ))}
        </div>
      </div>

      <div style={s.section}>
        <div style={s.head}>Add</div>
        <div style={s.add}>
          <button style={s.addBtn} onClick={() => addElement('plane', `Plane ${(elements.length + 1)}`, { color: '#10101e', width: 3, height: 2 })}>
            <Square size={13} /> Plane
          </button>
          <button style={s.addBtn} onClick={() => addElement('box', `Box ${(elements.length + 1)}`, { color: '#4488ff' })}>
            <Box size={13} /> Box
          </button>
          <button style={s.addBtn} onClick={() => addElement('sphere', `Sphere ${(elements.length + 1)}`, { color: '#ff4488' })}>
            <Circle size={13} /> Sphere
          </button>
        </div>
      </div>
    </div>
  )
}

const s: Record<string, React.CSSProperties> = {
  sidebar: { display: 'flex', flexDirection: 'column', height: '100%', background: '#111116', color: '#c8c8d4', fontSize: 12, fontFamily: "'Inter', sans-serif", borderRight: '1px solid #1e1e28' },
  section: { borderBottom: '1px solid #1a1a22' },
  head: { display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '7px 12px', fontSize: 10, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.09em', color: '#555570', background: '#0f0f14' },
  badge: { background: '#1e1e2a', borderRadius: 10, padding: '1px 5px', fontSize: 10, color: '#666688' },
  list: { maxHeight: 320, overflowY: 'auto', padding: '3px 0' },
  empty: { padding: '16px 12px', color: '#333344', textAlign: 'center', fontSize: 11, lineHeight: 1.5 },
  row: { display: 'flex', alignItems: 'center', gap: 6, padding: '4px 8px 4px 12px', cursor: 'pointer', borderLeft: '2px solid transparent' },
  rowSel: { background: '#1a1a2e', borderLeftColor: '#6644ff' },
  icon: { color: '#666688', flexShrink: 0, display: 'flex' },
  name: { flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' },
  actions: { display: 'flex', gap: 2 },
  iconBtn: { background: 'none', border: 'none', color: '#666688', cursor: 'pointer', padding: 2, borderRadius: 3, display: 'flex', alignItems: 'center' },
  add: { display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 5, padding: 10 },
  addBtn: { display: 'flex', alignItems: 'center', gap: 4, padding: '7px 6px', background: '#1a1a22', border: '1px solid #222230', borderRadius: 5, color: '#b0b0c8', fontSize: 11, cursor: 'pointer' },
}
