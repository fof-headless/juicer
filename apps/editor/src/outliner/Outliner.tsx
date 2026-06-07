import { useSceneStore, SceneElement, ElementType } from '../store/scene'
import { Eye, EyeOff, Lock, Unlock, Trash2, Box, Circle, Type, Image, Code, Layers } from 'lucide-react'
import './Outliner.css'

const TYPE_ICONS: Record<ElementType, React.ReactNode> = {
  'html-plane': <Code size={12} />,
  'image-plane': <Image size={12} />,
  text: <Type size={12} />,
  box: <Box size={12} />,
  sphere: <Circle size={12} />,
  group: <Layers size={12} />,
}

export function Outliner() {
  const elements = useSceneStore((s) => s.elements)
  const selectedIds = useSceneStore((s) => s.selectedIds)
  const selectElement = useSceneStore((s) => s.selectElement)
  const updateElement = useSceneStore((s) => s.updateElement)
  const removeElement = useSceneStore((s) => s.removeElement)

  const items = Object.values(elements).filter((e) => !e.parentId)

  return (
    <div className="outliner">
      <div className="outliner-header">
        <span>Scene</span>
        <span className="outliner-count">{Object.keys(elements).length}</span>
      </div>
      <div className="outliner-list">
        {items.length === 0 && (
          <div className="outliner-empty">No elements. Add one from the toolbar or paste HTML.</div>
        )}
        {items.map((el) => (
          <OutlinerRow
            key={el.id}
            el={el}
            isSelected={selectedIds.includes(el.id)}
            onSelect={(e) => selectElement(el.id, e.shiftKey)}
            onToggleVisible={() => updateElement(el.id, { visible: !el.visible })}
            onToggleLocked={() => updateElement(el.id, { locked: !el.locked })}
            onDelete={() => removeElement(el.id)}
          />
        ))}
      </div>
    </div>
  )
}

interface RowProps {
  el: SceneElement
  isSelected: boolean
  onSelect: (e: React.MouseEvent) => void
  onToggleVisible: () => void
  onToggleLocked: () => void
  onDelete: () => void
}

function OutlinerRow({ el, isSelected, onSelect, onToggleVisible, onToggleLocked, onDelete }: RowProps) {
  return (
    <div className={`outliner-row ${isSelected ? 'selected' : ''} ${!el.visible ? 'hidden' : ''}`} onClick={onSelect}>
      <span className="outliner-icon">{TYPE_ICONS[el.type]}</span>
      <span className="outliner-name">{el.name}</span>
      <div className="outliner-actions">
        <button onClick={(e) => { e.stopPropagation(); onToggleVisible() }} title="Toggle visibility">
          {el.visible ? <Eye size={12} /> : <EyeOff size={12} />}
        </button>
        <button onClick={(e) => { e.stopPropagation(); onToggleLocked() }} title="Toggle lock">
          {el.locked ? <Lock size={12} /> : <Unlock size={12} />}
        </button>
        <button onClick={(e) => { e.stopPropagation(); onDelete() }} title="Delete" className="delete-btn">
          <Trash2 size={12} />
        </button>
      </div>
    </div>
  )
}
