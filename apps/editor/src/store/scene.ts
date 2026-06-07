import { create } from 'zustand'
import { immer } from 'zustand/middleware/immer'
import * as THREE from 'three'

export type ElementType = 'html-plane' | 'image-plane' | 'text' | 'box' | 'sphere' | 'group'

export interface SceneElement {
  id: string
  name: string
  type: ElementType
  visible: boolean
  locked: boolean
  // Transform
  position: [number, number, number]
  rotation: [number, number, number]
  scale: [number, number, number]
  // Content
  htmlContent?: string
  imageUrl?: string
  textContent?: string
  color?: string
  opacity?: number
  // Dimensions (for planes)
  width?: number
  height?: number
  // Children (for groups)
  children?: string[]
  parentId?: string
}

export interface RecordingState {
  isRecording: boolean
  fps: number
  duration: number // seconds
  format: 'webm' | 'gif' | 'png-sequence'
}

export interface SceneState {
  elements: Record<string, SceneElement>
  selectedIds: string[]
  playhead: number // seconds
  isPlaying: boolean
  duration: number // total scene duration in seconds
  recording: RecordingState
  wsConnected: boolean

  // Actions
  addElement: (el: Omit<SceneElement, 'id'>) => string
  updateElement: (id: string, patch: Partial<SceneElement>) => void
  removeElement: (id: string) => void
  selectElement: (id: string, multi?: boolean) => void
  clearSelection: () => void
  setPlayhead: (t: number) => void
  setPlaying: (playing: boolean) => void
  setDuration: (d: number) => void
  setWsConnected: (v: boolean) => void
  setRecording: (patch: Partial<RecordingState>) => void
}

let idCounter = 1
const uid = () => `el_${Date.now()}_${idCounter++}`

export const useSceneStore = create<SceneState>()(
  immer((set) => ({
    elements: {},
    selectedIds: [],
    playhead: 0,
    isPlaying: false,
    duration: 10,
    wsConnected: false,
    recording: {
      isRecording: false,
      fps: 60,
      duration: 10,
      format: 'webm',
    },

    addElement: (el) => {
      const id = uid()
      set((s) => {
        s.elements[id] = { ...el, id }
      })
      return id
    },

    updateElement: (id, patch) => {
      set((s) => {
        if (s.elements[id]) {
          Object.assign(s.elements[id], patch)
        }
      })
    },

    removeElement: (id) => {
      set((s) => {
        delete s.elements[id]
        s.selectedIds = s.selectedIds.filter((x) => x !== id)
      })
    },

    selectElement: (id, multi = false) => {
      set((s) => {
        if (multi) {
          if (s.selectedIds.includes(id)) {
            s.selectedIds = s.selectedIds.filter((x) => x !== id)
          } else {
            s.selectedIds.push(id)
          }
        } else {
          s.selectedIds = [id]
        }
      })
    },

    clearSelection: () => {
      set((s) => {
        s.selectedIds = []
      })
    },

    setPlayhead: (t) => {
      set((s) => {
        s.playhead = t
      })
    },

    setPlaying: (playing) => {
      set((s) => {
        s.isPlaying = playing
      })
    },

    setDuration: (d) => {
      set((s) => {
        s.duration = d
      })
    },

    setWsConnected: (v) => {
      set((s) => {
        s.wsConnected = v
      })
    },

    setRecording: (patch) => {
      set((s) => {
        Object.assign(s.recording, patch)
      })
    },
  }))
)
