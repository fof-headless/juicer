import studio from '@theatre/studio'
import { getProject, types } from '@theatre/core'

// Initialize Theatre.js studio (the Blender-like keyframe editor UI)
studio.initialize()

export const project = getProject('Juicer Scene')

export const mainSheet = project.sheet('Main Sequence')

// Create a Theatre object for a scene element
export function createTheatreObject(
  elementId: string,
  name: string,
  initialValues: {
    position: { x: number; y: number; z: number }
    rotation: { x: number; y: number; z: number }
    scale: { x: number; y: number; z: number }
    opacity: number
  }
) {
  return mainSheet.object(elementId, {
    label: name,
    props: {
      position: types.compound({
        x: types.number(initialValues.position.x, { range: [-20, 20] }),
        y: types.number(initialValues.position.y, { range: [-20, 20] }),
        z: types.number(initialValues.position.z, { range: [-20, 20] }),
      }),
      rotation: types.compound({
        x: types.number(initialValues.rotation.x, { range: [-Math.PI, Math.PI] }),
        y: types.number(initialValues.rotation.y, { range: [-Math.PI, Math.PI] }),
        z: types.number(initialValues.rotation.z, { range: [-Math.PI, Math.PI] }),
      }),
      scale: types.compound({
        x: types.number(initialValues.scale.x, { range: [0.01, 10] }),
        y: types.number(initialValues.scale.y, { range: [0.01, 10] }),
        z: types.number(initialValues.scale.z, { range: [0.01, 10] }),
      }),
      opacity: types.number(initialValues.opacity, { range: [0, 1] }),
    },
  })
}

export { studio }
