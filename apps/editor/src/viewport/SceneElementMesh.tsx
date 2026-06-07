import { useRef, useEffect, useState, useCallback } from 'react'
import { useFrame } from '@react-three/fiber'
import { Text, RoundedBox } from '@react-three/drei'
import * as THREE from 'three'
import { SceneElement } from '../store/scene'
import { useSceneStore } from '../store/scene'
import { createTheatreObject } from '../store/theatre'
import { useCurrentSheet } from '@theatre/r3f'
import { editable as e } from '@theatre/r3f'

interface Props {
  element: SceneElement
}

export function SceneElementMesh({ element }: Props) {
  const meshRef = useRef<THREE.Mesh>(null)
  const selectedIds = useSceneStore((s) => s.selectedIds)
  const selectElement = useSceneStore((s) => s.selectElement)
  const isSelected = selectedIds.includes(element.id)

  const [texture, setTexture] = useState<THREE.Texture | null>(null)
  const [theatreObj] = useState(() =>
    createTheatreObject(element.id, element.name, {
      position: { x: element.position[0], y: element.position[1], z: element.position[2] },
      rotation: { x: element.rotation[0], y: element.rotation[1], z: element.rotation[2] },
      scale: { x: element.scale[0], y: element.scale[1], z: element.scale[2] },
      opacity: element.opacity ?? 1,
    })
  )

  // Sync Theatre.js values to mesh every frame
  useFrame(() => {
    if (!meshRef.current) return
    const v = theatreObj.value
    meshRef.current.position.set(v.position.x, v.position.y, v.position.z)
    meshRef.current.rotation.set(v.rotation.x, v.rotation.y, v.rotation.z)
    meshRef.current.scale.set(v.scale.x, v.scale.y, v.scale.z)
    if (meshRef.current.material instanceof THREE.MeshStandardMaterial) {
      meshRef.current.material.opacity = v.opacity
      meshRef.current.material.transparent = v.opacity < 1
    }
  })

  // Load HTML snapshot as texture
  useEffect(() => {
    if (element.type === 'html-plane' && element.htmlContent) {
      renderHtmlToTexture(element.htmlContent, element.width ?? 800, element.height ?? 600).then(
        setTexture
      )
    } else if (element.type === 'image-plane' && element.imageUrl) {
      const loader = new THREE.TextureLoader()
      loader.load(element.imageUrl, setTexture)
    }
  }, [element.type, element.htmlContent, element.imageUrl, element.width, element.height])

  const handleClick = useCallback(
    (e: any) => {
      e.stopPropagation()
      selectElement(element.id, e.shiftKey)
    },
    [element.id, selectElement]
  )

  if (element.type === 'text') {
    return (
      <Text
        ref={meshRef as any}
        position={element.position}
        rotation={element.rotation as any}
        scale={element.scale}
        color={element.color ?? '#ffffff'}
        fontSize={0.5}
        maxWidth={10}
        textAlign="center"
        onClick={handleClick}
      >
        {element.textContent ?? 'Text'}
        {isSelected && (
          <meshStandardMaterial color={element.color ?? '#ffffff'} emissive="#6644ff" emissiveIntensity={0.3} />
        )}
      </Text>
    )
  }

  if (element.type === 'box') {
    return (
      <mesh ref={meshRef} position={element.position} rotation={element.rotation as any} onClick={handleClick} castShadow receiveShadow>
        <boxGeometry args={[element.width ?? 1, element.height ?? 1, 0.1]} />
        <meshStandardMaterial
          color={isSelected ? '#6644ff' : (element.color ?? '#4488ff')}
          transparent
          opacity={element.opacity ?? 1}
          emissive={isSelected ? '#6644ff' : '#000000'}
          emissiveIntensity={isSelected ? 0.2 : 0}
        />
      </mesh>
    )
  }

  if (element.type === 'sphere') {
    return (
      <mesh ref={meshRef} position={element.position} rotation={element.rotation as any} onClick={handleClick} castShadow>
        <sphereGeometry args={[element.width ?? 0.5, 32, 32]} />
        <meshStandardMaterial
          color={isSelected ? '#6644ff' : (element.color ?? '#ff4488')}
          transparent
          opacity={element.opacity ?? 1}
          emissive={isSelected ? '#6644ff' : '#000000'}
          emissiveIntensity={isSelected ? 0.2 : 0}
        />
      </mesh>
    )
  }

  // HTML plane or image plane
  const w = element.width ?? 4
  const h = element.height ?? 2.5

  return (
    <mesh
      ref={meshRef}
      position={element.position}
      rotation={element.rotation as any}
      onClick={handleClick}
      castShadow
    >
      <planeGeometry args={[w, h]} />
      <meshStandardMaterial
        map={texture}
        color={texture ? '#ffffff' : (element.color ?? '#334455')}
        transparent
        opacity={element.opacity ?? 1}
        side={THREE.DoubleSide}
        emissive={isSelected ? '#6644ff' : '#000000'}
        emissiveIntensity={isSelected ? 0.15 : 0}
      />
      {!texture && (
        <>
          {/* Placeholder label */}
        </>
      )}
    </mesh>
  )
}

async function renderHtmlToTexture(html: string, w: number, h: number): Promise<THREE.Texture> {
  // Render HTML to an offscreen canvas via foreignObject SVG trick
  const svg = `
    <svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}">
      <foreignObject width="100%" height="100%">
        <div xmlns="http://www.w3.org/1999/xhtml" style="width:${w}px;height:${h}px;overflow:hidden;">
          ${html}
        </div>
      </foreignObject>
    </svg>
  `
  const blob = new Blob([svg], { type: 'image/svg+xml' })
  const url = URL.createObjectURL(blob)
  const img = new Image()
  img.src = url
  await new Promise((res) => (img.onload = res))

  const canvas = document.createElement('canvas')
  canvas.width = w
  canvas.height = h
  const ctx = canvas.getContext('2d')!
  ctx.drawImage(img, 0, 0)
  URL.revokeObjectURL(url)

  const tex = new THREE.CanvasTexture(canvas)
  tex.needsUpdate = true
  return tex
}
