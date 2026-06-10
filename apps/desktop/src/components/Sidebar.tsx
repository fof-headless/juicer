import { useSceneStore, LayerKind } from '../store/scene'
import {
  Square,
  Image as ImageIcon,
  Type,
  Code,
  Eye,
  EyeOff,
  Trash2,
  Copy,
  ChevronUp,
  ChevronDown,
} from 'lucide-react'

const KIND_ICON: Record<LayerKind, React.ReactNode> = {
  shape: <Square size={11} />,
  image: <ImageIcon size={11} />,
  text: <Type size={11} />,
  html: <Code size={11} />,
}

export function Sidebar() {
  const scene = useSceneStore((s) => s.scene)
  const selectedId = useSceneStore((s) => s.selectedId)
  const select = useSceneStore((s) => s.select)
  const removeLayer = useSceneStore((s) => s.removeLayer)
  const setVisible = useSceneStore((s) => s.setVisible)
  const duplicateLayer = useSceneStore((s) => s.duplicateLayer)
  const reorderLayer = useSceneStore((s) => s.reorderLayer)
  const addShapeLayer = useSceneStore((s) => s.addShapeLayer)
  const addTextLayer = useSceneStore((s) => s.addTextLayer)

  if (!scene) return <div style={styles.empty}>Loading…</div>

  // Render in REVERSE order so the top of the list is the front-most layer
  // (Figma convention).
  const ordered = scene.layers.map((l, i) => ({ layer: l, z: i })).reverse()

  return (
    <div style={styles.root}>
      <div style={styles.header}>
        <span>LAYERS</span>
        <div style={{ marginLeft: 'auto', display: 'flex', gap: 4 }}>
          <button
            title="Add rectangle"
            style={styles.btn}
            onClick={() => addShapeLayer('rect')}
          >
            <Square size={11} />
          </button>
          <button
            title="Add text"
            style={styles.btn}
            onClick={() => addTextLayer('Heading')}
          >
            <Type size={11} />
          </button>
        </div>
      </div>
      <div style={styles.list}>
        {ordered.length === 0 && (
          <div style={styles.placeholder}>
            No layers yet. Add a shape, text, or paste HTML in the panel below.
          </div>
        )}
        {ordered.map(({ layer, z }) => {
          const selected = layer.id === selectedId
          const kind = (layer.kind as { type: LayerKind }).type
          return (
            <div
              key={layer.id}
              style={{ ...styles.row, ...(selected ? styles.rowSelected : null) }}
              onClick={() => select(layer.id)}
            >
              <span style={styles.kind}>{KIND_ICON[kind]}</span>
              <span style={styles.name}>{layer.name}</span>
              <button
                style={styles.iconBtn}
                title={layer.visible ? 'Hide' : 'Show'}
                onClick={(e) => {
                  e.stopPropagation()
                  setVisible(layer.id, !layer.visible)
                }}
              >
                {layer.visible ? <Eye size={11} /> : <EyeOff size={11} />}
              </button>
              <button
                style={styles.iconBtn}
                title="Move up (toward front)"
                onClick={(e) => {
                  e.stopPropagation()
                  reorderLayer(layer.id, z + 1)
                }}
                disabled={z >= scene.layers.length - 1}
              >
                <ChevronUp size={11} />
              </button>
              <button
                style={styles.iconBtn}
                title="Move down (toward back)"
                onClick={(e) => {
                  e.stopPropagation()
                  reorderLayer(layer.id, z - 1)
                }}
                disabled={z === 0}
              >
                <ChevronDown size={11} />
              </button>
              <button
                style={styles.iconBtn}
                title="Duplicate"
                onClick={(e) => {
                  e.stopPropagation()
                  duplicateLayer(layer.id)
                }}
              >
                <Copy size={11} />
              </button>
              <button
                style={styles.iconBtn}
                title="Delete"
                onClick={(e) => {
                  e.stopPropagation()
                  removeLayer(layer.id)
                }}
              >
                <Trash2 size={11} />
              </button>
            </div>
          )
        })}
      </div>
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
    overflow: 'hidden',
  },
  header: {
    padding: '8px 12px',
    fontSize: 10,
    fontWeight: 600,
    letterSpacing: 1,
    color: '#888',
    borderBottom: '1px solid #2a2a36',
    display: 'flex',
    alignItems: 'center',
    gap: 8,
  },
  list: {
    flex: 1,
    minHeight: 0,
    overflowY: 'auto',
  },
  row: {
    display: 'flex',
    alignItems: 'center',
    gap: 6,
    padding: '5px 8px',
    cursor: 'pointer',
    fontSize: 12,
    color: '#cfcfdc',
    borderBottom: '1px solid #1c1c25',
  },
  rowSelected: {
    background: '#272739',
    color: '#fff',
  },
  kind: {
    width: 14,
    display: 'inline-flex',
    justifyContent: 'center',
    color: '#888',
  },
  name: {
    flex: 1,
    whiteSpace: 'nowrap',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
  },
  iconBtn: {
    background: 'transparent',
    border: 0,
    color: '#888',
    cursor: 'pointer',
    padding: 2,
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
  },
  btn: {
    background: '#1f1f29',
    border: '1px solid #2a2a36',
    color: '#cfcfdc',
    cursor: 'pointer',
    padding: '2px 6px',
    borderRadius: 3,
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
    fontSize: 11,
  },
  empty: {
    padding: 12,
    color: '#888',
    fontSize: 12,
  },
  placeholder: {
    padding: 12,
    color: '#666',
    fontSize: 11,
    fontStyle: 'italic',
  },
}
