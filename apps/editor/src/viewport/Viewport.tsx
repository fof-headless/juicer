import { Canvas } from '@react-three/fiber'
import { OrbitControls, Grid, GizmoHelper, GizmoViewport, Environment, Stats } from '@react-three/drei'
import { SheetProvider } from '@theatre/r3f'
import { Suspense, useRef } from 'react'
import { useSceneStore } from '../store/scene'
import { mainSheet } from '../store/theatre'
import { SceneElementMesh } from './SceneElementMesh'
import { SelectionGizmo } from './SelectionGizmo'
import * as THREE from 'three'

export function Viewport() {
  const elements = useSceneStore((s) => s.elements)
  const clearSelection = useSceneStore((s) => s.clearSelection)

  return (
    <div style={{ width: '100%', height: '100%', background: '#1a1a2e' }}>
      <Canvas
        shadows
        camera={{ position: [0, 3, 8], fov: 50 }}
        gl={{ preserveDrawingBuffer: true, antialias: true }}
        onPointerMissed={() => clearSelection()}
      >
        <SheetProvider sheet={mainSheet}>
          <Suspense fallback={null}>
            {/* Lighting */}
            <ambientLight intensity={0.6} />
            <directionalLight
              position={[5, 10, 5]}
              intensity={1.2}
              castShadow
              shadow-mapSize={[2048, 2048]}
            />
            <directionalLight position={[-5, 5, -5]} intensity={0.4} />

            {/* Ground grid */}
            <Grid
              args={[30, 30]}
              cellSize={0.5}
              cellThickness={0.5}
              cellColor="#2a2a4a"
              sectionSize={2}
              sectionThickness={1}
              sectionColor="#4a4a8a"
              fadeDistance={25}
              fadeStrength={1}
              followCamera={false}
              infiniteGrid
            />

            {/* Scene elements */}
            {Object.values(elements).map((el) => (
              <SceneElementMesh key={el.id} element={el} />
            ))}

            {/* Transform gizmo for selected */}
            <SelectionGizmo />

            {/* Controls */}
            <OrbitControls
              makeDefault
              enableDamping
              dampingFactor={0.05}
              screenSpacePanning
            />

            {/* Viewport cube helper */}
            <GizmoHelper alignment="bottom-right" margin={[80, 80]}>
              <GizmoViewport labelColor="white" axisHeadScale={1} />
            </GizmoHelper>

            <Stats />
          </Suspense>
        </SheetProvider>
      </Canvas>
    </div>
  )
}
