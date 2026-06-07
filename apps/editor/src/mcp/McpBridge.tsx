import { useEffect, useRef } from 'react'
import { useSceneStore } from '../store/scene'
import { mainSheet } from '../store/theatre'
import { createTheatreObject } from '../store/theatre'

// Commands Claude can send over WebSocket
type McpCommand =
  | { type: 'add_element'; payload: Parameters<typeof useSceneStore.getState>['0'] extends infer S ? any : any }
  | { type: 'update_element'; payload: { id: string; patch: any } }
  | { type: 'remove_element'; payload: { id: string } }
  | { type: 'select_element'; payload: { id: string } }
  | { type: 'set_keyframe'; payload: { elementId: string; time: number; property: string; value: any } }
  | { type: 'play' }
  | { type: 'pause' }
  | { type: 'seek'; payload: { time: number } }
  | { type: 'get_scene' }

export function McpBridge() {
  const wsRef = useRef<WebSocket | null>(null)
  const setWsConnected = useSceneStore((s) => s.setWsConnected)
  const addElement = useSceneStore((s) => s.addElement)
  const updateElement = useSceneStore((s) => s.updateElement)
  const removeElement = useSceneStore((s) => s.removeElement)
  const selectElement = useSceneStore((s) => s.selectElement)
  const setPlaying = useSceneStore((s) => s.setPlaying)
  const setPlayhead = useSceneStore((s) => s.setPlayhead)

  useEffect(() => {
    let reconnectTimer: ReturnType<typeof setTimeout>

    const connect = () => {
      const ws = new WebSocket('ws://localhost:3001')
      wsRef.current = ws

      ws.onopen = () => {
        setWsConnected(true)
        console.log('[MCP Bridge] Connected to MCP server')
      }

      ws.onclose = () => {
        setWsConnected(false)
        reconnectTimer = setTimeout(connect, 3000)
      }

      ws.onerror = () => {
        setWsConnected(false)
      }

      ws.onmessage = (evt) => {
        try {
          const cmd: McpCommand = JSON.parse(evt.data)
          handleCommand(cmd, ws)
        } catch (e) {
          console.warn('[MCP Bridge] Bad message', evt.data)
        }
      }
    }

    connect()
    return () => {
      clearTimeout(reconnectTimer)
      wsRef.current?.close()
    }
  }, [])

  function handleCommand(cmd: McpCommand, ws: WebSocket) {
    const state = useSceneStore.getState()

    switch (cmd.type) {
      case 'add_element': {
        const id = addElement({
          name: cmd.payload.name ?? 'New Element',
          type: cmd.payload.type ?? 'box',
          visible: true,
          locked: false,
          position: cmd.payload.position ?? [0, 0, 0],
          rotation: cmd.payload.rotation ?? [0, 0, 0],
          scale: cmd.payload.scale ?? [1, 1, 1],
          color: cmd.payload.color,
          opacity: cmd.payload.opacity ?? 1,
          htmlContent: cmd.payload.htmlContent,
          imageUrl: cmd.payload.imageUrl,
          textContent: cmd.payload.textContent,
          width: cmd.payload.width ?? 2,
          height: cmd.payload.height ?? 2,
        })
        ws.send(JSON.stringify({ type: 'ack', id, command: cmd.type }))
        break
      }

      case 'update_element': {
        updateElement(cmd.payload.id, cmd.payload.patch)
        ws.send(JSON.stringify({ type: 'ack', command: cmd.type }))
        break
      }

      case 'remove_element': {
        removeElement(cmd.payload.id)
        ws.send(JSON.stringify({ type: 'ack', command: cmd.type }))
        break
      }

      case 'select_element': {
        selectElement(cmd.payload.id)
        ws.send(JSON.stringify({ type: 'ack', command: cmd.type }))
        break
      }

      case 'play': {
        mainSheet.sequence.play()
        setPlaying(true)
        ws.send(JSON.stringify({ type: 'ack', command: cmd.type }))
        break
      }

      case 'pause': {
        mainSheet.sequence.pause()
        setPlaying(false)
        ws.send(JSON.stringify({ type: 'ack', command: cmd.type }))
        break
      }

      case 'seek': {
        mainSheet.sequence.position = cmd.payload.time
        setPlayhead(cmd.payload.time)
        ws.send(JSON.stringify({ type: 'ack', command: cmd.type }))
        break
      }

      case 'get_scene': {
        const elements = state.elements
        ws.send(
          JSON.stringify({
            type: 'scene_data',
            data: {
              elements: Object.values(elements),
              duration: state.duration,
              playhead: state.playhead,
              isPlaying: state.isPlaying,
            },
          })
        )
        break
      }
    }
  }

  return null
}
