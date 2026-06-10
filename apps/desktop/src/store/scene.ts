import { create } from 'zustand'
import { immer } from 'zustand/middleware/immer'
import { invoke } from '@tauri-apps/api/core'

// ── Types mirroring scene.rs ──────────────────────────────────────────────────

export type LayerKind = 'html' | 'image' | 'shape' | 'text'

export interface NamedTrack {
  property: string
  track: { keys: Array<{ frame: number; value: unknown; easing: unknown }> }
}

export interface Transform2_5D {
  x: number
  y: number
  rotation: number
  scale_x: number
  scale_y: number
  rotate_x: number
  rotate_y: number
  perspective: number
  origin_x: number
  origin_y: number
}

export interface BoxShadow {
  offset_x: number
  offset_y: number
  blur: number
  spread: number
  color: string
  inset: boolean
}

export interface Effects {
  shadows: BoxShadow[]
  filter_blur: number
}

export interface Layer {
  id: string
  name: string
  kind: { type: LayerKind } & Record<string, unknown>
  width: number
  height: number
  transform: Transform2_5D
  opacity: number
  visible: boolean
  blend_mode: string
  effects: Effects
  tracks: NamedTrack[]
}

export interface Canvas {
  width: number
  height: number
  fps: number
  background: string
}

export interface SceneData {
  layers: Layer[]
  canvas: Canvas
  duration_frames: number
}

export interface ProjectInfo {
  name: string
  root: string
  assets: string
  renders: string
}

// ── Store ─────────────────────────────────────────────────────────────────────

interface State {
  scene: SceneData | null
  project: ProjectInfo | null
  selectedId: string | null
  frame: number
  playing: boolean
  isRendering: boolean

  // High-level
  refresh: () => Promise<void>
  refreshProject: () => Promise<void>
  call: <T = unknown>(tool: string, args?: Record<string, unknown>) => Promise<T>
  setFrame: (f: number) => void
  select: (id: string | null) => void
  setPlaying: (p: boolean) => void

  // Convenience wrappers
  addHtmlLayer: (html: string, name?: string) => Promise<string>
  addImageLayer: (srcPath: string, name?: string) => Promise<string>
  addShapeLayer: (
    shape: 'rect' | 'ellipse',
    fillColor?: string,
    name?: string,
  ) => Promise<string>
  addTextLayer: (text: string, name?: string) => Promise<string>
  removeLayer: (id: string) => Promise<void>
  duplicateLayer: (id: string) => Promise<string>
  reorderLayer: (id: string, zIndex: number) => Promise<void>

  setTransform: (id: string, patch: Partial<Transform2_5D>) => Promise<void>
  setOpacity: (id: string, opacity: number) => Promise<void>
  setSize: (id: string, w?: number, h?: number) => Promise<void>
  setBorderRadius: (id: string, radius: number) => Promise<void>
  setShadow: (id: string, shadow: Partial<BoxShadow>, index?: number) => Promise<void>
  clearShadows: (id: string) => Promise<void>
  setBlur: (id: string, radius: number) => Promise<void>
  setVisible: (id: string, visible: boolean) => Promise<void>
  renameLayer: (id: string, name: string) => Promise<void>

  setKeyframe: (
    id: string,
    frame: number,
    property: string,
    value: unknown,
    easing?: string,
  ) => Promise<void>
  removeKeyframe: (id: string, frame: number, property: string) => Promise<void>

  setCanvas: (patch: Partial<Canvas>) => Promise<void>
  setDuration: (frames: number) => Promise<void>

  renderFrame: (frame?: number) => Promise<{ path: string; image_base64: string }>
  renderAnimation: (outputPath?: string) => Promise<string>

  createProject: (name: string) => Promise<void>
  openProject: (path: string) => Promise<void>
  saveProject: () => Promise<string>
}

export const useSceneStore = create<State>()(
  immer((set, get) => ({
    scene: null,
    project: null,
    selectedId: null,
    frame: 0,
    playing: false,
    isRendering: false,

    call: async <T,>(tool: string, args: Record<string, unknown> = {}) => {
      return (await invoke<T>('call', { tool, args })) as T
    },

    refresh: async () => {
      try {
        const scene = await invoke<SceneData>('get_scene_cmd')
        set((s) => {
          s.scene = scene
          // Clear selection if the layer's gone.
          if (s.selectedId && !scene.layers.some((l) => l.id === s.selectedId)) {
            s.selectedId = null
          }
        })
      } catch (e) {
        console.error('refresh', e)
      }
    },

    refreshProject: async () => {
      try {
        const project = await get().call<ProjectInfo | null>('get_project')
        set((s) => { s.project = project })
      } catch (e) {
        console.error('refreshProject', e)
      }
    },

    setFrame: (f) => set((s) => { s.frame = f }),
    select: (id) => set((s) => { s.selectedId = id }),
    setPlaying: (p) => set((s) => { s.playing = p }),

    addHtmlLayer: async (html, name) => {
      const r = await get().call<{ id: string }>('add_html_layer', { html, name })
      await get().refresh()
      set((s) => { s.selectedId = r.id })
      return r.id
    },
    addImageLayer: async (src_path, name) => {
      const r = await get().call<{ id: string }>('add_image_layer', { src_path, name })
      await get().refresh()
      set((s) => { s.selectedId = r.id })
      return r.id
    },
    addShapeLayer: async (shape, fillColor = '#6644ff', name) => {
      const r = await get().call<{ id: string }>('add_shape_layer', {
        shape,
        fill: { type: 'solid', color: fillColor },
        name,
      })
      await get().refresh()
      set((s) => { s.selectedId = r.id })
      return r.id
    },
    addTextLayer: async (text, name) => {
      const r = await get().call<{ id: string }>('add_text_layer', { text, name })
      await get().refresh()
      set((s) => { s.selectedId = r.id })
      return r.id
    },
    removeLayer: async (id) => {
      await get().call('remove_layer', { id })
      set((s) => { if (s.selectedId === id) s.selectedId = null })
      await get().refresh()
    },
    duplicateLayer: async (id) => {
      const r = await get().call<{ id: string }>('duplicate_layer', { id })
      await get().refresh()
      set((s) => { s.selectedId = r.id })
      return r.id
    },
    reorderLayer: async (id, z_index) => {
      await get().call('reorder_layer', { id, z_index })
      await get().refresh()
    },

    setTransform: async (id, patch) => {
      await get().call('set_transform', { id, ...patch })
      await get().refresh()
    },
    setOpacity: async (id, opacity) => {
      await get().call('set_opacity', { id, opacity })
      await get().refresh()
    },
    setSize: async (id, width, height) => {
      await get().call('set_size', { id, width, height })
      await get().refresh()
    },
    setBorderRadius: async (id, radius) => {
      await get().call('set_border_radius', { id, radius })
      await get().refresh()
    },
    setShadow: async (id, shadow, index) => {
      await get().call('set_shadow', { id, ...shadow, index })
      await get().refresh()
    },
    clearShadows: async (id) => {
      await get().call('clear_shadows', { id })
      await get().refresh()
    },
    setBlur: async (id, radius) => {
      await get().call('set_blur', { id, radius })
      await get().refresh()
    },
    setVisible: async (id, visible) => {
      await get().call('set_visible', { id, visible })
      await get().refresh()
    },
    renameLayer: async (id, name) => {
      await get().call('rename_layer', { id, name })
      await get().refresh()
    },

    setKeyframe: async (id, frame, property, value, easing = 'ease-in-out') => {
      await get().call('set_keyframe', { id, frame, property, value, easing })
      await get().refresh()
    },
    removeKeyframe: async (id, frame, property) => {
      await get().call('remove_keyframe', { id, frame, property })
      await get().refresh()
    },

    setCanvas: async (patch) => {
      await get().call('set_canvas', patch as Record<string, unknown>)
      await get().refresh()
    },
    setDuration: async (frames) => {
      await get().call('set_duration', { frames })
      await get().refresh()
    },

    renderFrame: async (frame) => {
      const f = frame ?? get().frame
      return await get().call<{ path: string; image_base64: string }>(
        'render_frame', { frame: f })
    },
    renderAnimation: async (outputPath) => {
      set((s) => { s.isRendering = true })
      try {
        const r = await get().call<{ path: string }>('render_animation', {
          output_path: outputPath,
        })
        return r.path
      } finally {
        set((s) => { s.isRendering = false })
      }
    },

    createProject: async (name) => {
      const project = await get().call<ProjectInfo>('create_project', { name })
      set((s) => { s.project = project; s.selectedId = null })
      await get().refresh()
    },
    openProject: async (path) => {
      const project = await get().call<ProjectInfo>('open_project', { path })
      set((s) => { s.project = project; s.selectedId = null })
      await get().refresh()
    },
    saveProject: async () => {
      const r = await get().call<{ path: string }>('save_project')
      await get().refreshProject()
      return r.path
    },
  })),
)

// Helper for components: get the selected layer (or null).
export function useSelectedLayer(): Layer | null {
  const scene = useSceneStore((s) => s.scene)
  const id = useSceneStore((s) => s.selectedId)
  if (!scene || !id) return null
  return scene.layers.find((l) => l.id === id) || null
}
