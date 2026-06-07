import { useRef } from 'react'
import { TransformControls } from '@react-three/drei'
import { useSceneStore } from '../store/scene'
import * as THREE from 'three'

export function SelectionGizmo() {
  const selectedIds = useSceneStore((s) => s.selectedIds)
  const elements = useSceneStore((s) => s.elements)
  const updateElement = useSceneStore((s) => s.updateElement)
  const ref = useRef<THREE.Object3D>(null)

  const selected = selectedIds.length === 1 ? elements[selectedIds[0]] : null

  if (!selected) return null

  return (
    <TransformControls
      object={ref as any}
      mode="translate"
      onObjectChange={(e: any) => {
        if (!ref.current) return
        const p = ref.current.position
        updateElement(selected.id, {
          position: [p.x, p.y, p.z],
        })
      }}
    >
      <object3D
        ref={ref}
        position={selected.position as any}
      />
    </TransformControls>
  )
}
