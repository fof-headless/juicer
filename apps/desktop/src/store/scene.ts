import { create } from 'zustand'
import { immer } from 'zustand/middleware/immer'
import { invoke } from '@tauri-apps/api/core'

export type ElementKind = 'plane' | 'box' | 'sphere'

export interface NamedTrack {
  property: string
  track: { keys: Array<{ frame: number; value: number | [number, number, number]; easing: string }> }
}

export interface Element {
  id: string
  name: string
  kind: ElementKind
  position: [number, number, number]
  rotation: [number, number, number]
  scale: [number, number, number]
  visible: boolean
  color: string
  opacity: number
  image_path: string | null
  width: number
  height: number
  unlit: boolean
  tracks: NamedTrack[]
}

export interface RenderSettings {
  width: number
  height: number
  fps: number
  frame_start: number
  frame_end: number
  background: [number, number, number, number]
}

export interface Camera {
  position: [number, number, number]
  target: [number, number, number]
  fov_deg: number
  near: number
  far: number
  tracks: NamedTrack[]
}

export interface SceneData {
  elements: Element[]
  camera: Camera
  render: RenderSettings
  mode: string
}

interface State {
  scene: SceneData | null
  selectedId: string | null
  frame: number
  isRendering: boolean
  previewUrl: string | null
  previewVersion: number

  refresh: () => Promise<void>
  select: (id: string | null) => void
  setFrame: (f: number) => void

  addElement: (kind: ElementKind, name: string, extra?: Partial<Element>) => Promise<string>
  updateElement: (id: string, patch: Record<string, unknown>) => Promise<void>
  removeElement: (id: string) => Promise<void>
  setKeyframe: (id: string, frame: number, property: string, value: unknown, easing?: string) => Promise<void>
  renderPreview: (frame?: number) => Promise<void>
  renderVideo: (outputPath: string) => Promise<string>
  setRenderSettings: (patch: Partial<RenderSettings>) => Promise<void>
}

export const useSceneStore = create<State>()(
  immer((set, get) => ({
    scene: null,
    selectedId: null,
    frame: 1,
    isRendering: false,
    previewUrl: null,
    previewVersion: 0,

    refresh: async () => {
      try {
        const scene = await invoke<SceneData>('get_scene')
        set((s) => { s.scene = scene })
      } catch (e) {
        console.error('refresh', e)
      }
    },

    select: (id) => set((s) => { s.selectedId = id }),
    setFrame: (f) => set((s) => { s.frame = f }),

    addElement: async (kind, name, extra = {}) => {
      const id = await invoke<string>('add_element', {
        args: { kind, name, ...extra },
      })
      await get().refresh()
      return id
    },

    updateElement: async (id, patch) => {
      await invoke('update_element', { id, patch })
      await get().refresh()
    },

    removeElement: async (id) => {
      await invoke('remove_element', { id })
      set((s) => { if (s.selectedId === id) s.selectedId = null })
      await get().refresh()
    },

    setKeyframe: async (id, frame, property, value, easing = 'ease-in-out') => {
      await invoke('set_keyframe', { id, frame, property, value, easing })
      await get().refresh()
    },

    renderPreview: async (frame) => {
      const f = frame ?? get().frame
      try {
        const path = await invoke<string>('render_preview', { frame: f })
        // bust cache with version query
        set((s) => {
          s.previewVersion += 1
          s.previewUrl = `${convertFileSrc(path)}?v=${s.previewVersion}`
        })
      } catch (e) {
        console.error('preview', e)
      }
    },

    renderVideo: async (outputPath) => {
      set((s) => { s.isRendering = true })
      try {
        const out = await invoke<string>('render_video', { outputPath })
        return out
      } finally {
        set((s) => { s.isRendering = false })
      }
    },

    setRenderSettings: async (patch) => {
      await invoke('set_render_settings', patch)
      await get().refresh()
    },
  }))
)

// Tauri asset URL converter (lazy import to avoid SSR issues)
function convertFileSrc(path: string): string {
  // @ts-ignore - available in Tauri runtime
  if (typeof window !== 'undefined' && (window as any).__TAURI__?.core?.convertFileSrc) {
    return (window as any).__TAURI__.core.convertFileSrc(path)
  }
  return `file://${path}`
}
