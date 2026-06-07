import { create } from 'zustand'
import { immer } from 'zustand/middleware/immer'
import { invoke } from '@tauri-apps/api/core'

export interface BlenderObject {
  name: string
  type: string
  location: [number, number, number]
  rotation: [number, number, number]
  scale: [number, number, number]
  visible: boolean
  keyframes: Array<{ frame: number; data_path: string; array_index: number; value: number }>
}

export interface SceneState {
  objects: BlenderObject[]
  selectedName: string | null
  frame: number
  frameStart: number
  frameEnd: number
  fps: number
  blenderConnected: boolean
  isRendering: boolean
  renderProgress: string

  // Actions
  refreshScene: () => Promise<void>
  selectObject: (name: string | null) => void
  setFrame: (f: number) => void
  setBlenderConnected: (v: boolean) => void
  setRendering: (v: boolean, progress?: string) => void

  // Blender commands
  cmd: (op: string, params?: Record<string, unknown>) => Promise<unknown>
}

export const useSceneStore = create<SceneState>()(
  immer((set, get) => ({
    objects: [],
    selectedName: null,
    frame: 1,
    frameStart: 1,
    frameEnd: 300,
    fps: 30,
    blenderConnected: false,
    isRendering: false,
    renderProgress: '',

    cmd: async (op, params = {}) => {
      const payload = JSON.stringify({ op, ...params })
      try {
        const result = await invoke<unknown>('blender_cmd', { command: payload })
        return result
      } catch (e) {
        console.error(`[blender cmd] ${op}:`, e)
        throw e
      }
    },

    refreshScene: async () => {
      try {
        const result = await get().cmd('get_scene') as any
        set((s) => {
          s.objects = result.objects ?? []
          s.frame = result.frame_current ?? 1
          s.frameStart = result.frame_start ?? 1
          s.frameEnd = result.frame_end ?? 300
          s.fps = result.fps ?? 30
          s.blenderConnected = true
        })
      } catch {
        set((s) => { s.blenderConnected = false })
      }
    },

    selectObject: (name) => set((s) => { s.selectedName = name }),

    setFrame: (f) => set((s) => { s.frame = f }),

    setBlenderConnected: (v) => set((s) => { s.blenderConnected = v }),

    setRendering: (v, progress = '') =>
      set((s) => { s.isRendering = v; s.renderProgress = progress }),
  }))
)
