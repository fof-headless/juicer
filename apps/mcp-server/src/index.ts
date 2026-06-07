/**
 * Juicer MCP Server
 *
 * Two roles:
 * 1. MCP server (stdio) — Claude Desktop connects here via the MCP protocol
 * 2. WebSocket server (port 3001) — the browser editor connects here
 *
 * Claude Desktop sends tool calls → MCP server → WebSocket → Editor
 * Editor sends scene state back → WebSocket → MCP server → Claude Desktop
 */

import { Server } from '@modelcontextprotocol/sdk/server/index.js'
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js'
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from '@modelcontextprotocol/sdk/types.js'
import { WebSocketServer, WebSocket } from 'ws'
import { z } from 'zod'

// ── WebSocket server (browser editor connects here) ─────────────────────────
const wss = new WebSocketServer({ port: 3001 })
let editorSocket: WebSocket | null = null
const pendingResponses = new Map<string, (data: any) => void>()

wss.on('connection', (ws) => {
  editorSocket = ws
  console.error('[Juicer MCP] Editor connected')

  ws.on('message', (raw) => {
    try {
      const msg = JSON.parse(raw.toString())
      if (msg.type === 'scene_data' || msg.type === 'ack') {
        const resolver = pendingResponses.get(msg.type === 'scene_data' ? 'get_scene' : msg.command)
        if (resolver) {
          pendingResponses.delete(msg.type === 'scene_data' ? 'get_scene' : msg.command)
          resolver(msg)
        }
      }
    } catch {}
  })

  ws.on('close', () => {
    editorSocket = null
    console.error('[Juicer MCP] Editor disconnected')
  })
})

console.error('[Juicer MCP] WebSocket server on ws://localhost:3001')

// ── Helpers ──────────────────────────────────────────────────────────────────
function sendToEditor(cmd: object): Promise<any> {
  return new Promise((resolve, reject) => {
    if (!editorSocket || editorSocket.readyState !== WebSocket.OPEN) {
      reject(new Error('Juicer editor is not open. Start it with: pnpm editor'))
      return
    }
    const parsed = cmd as any
    const key = parsed.type === 'get_scene' ? 'get_scene' : parsed.type
    pendingResponses.set(key, resolve)
    editorSocket.send(JSON.stringify(cmd))
    setTimeout(() => {
      if (pendingResponses.has(key)) {
        pendingResponses.delete(key)
        reject(new Error('Timeout waiting for editor response'))
      }
    }, 5000)
  })
}

async function sendCommand(type: string, payload?: object): Promise<string> {
  const result = await sendToEditor({ type, ...(payload ? { payload } : {}) })
  return JSON.stringify(result)
}

// ── MCP Server ────────────────────────────────────────────────────────────────
const server = new Server(
  { name: 'juicer', version: '0.1.0' },
  { capabilities: { tools: {} } }
)

server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: [
    {
      name: 'get_scene',
      description: 'Get the current state of the Juicer 3D scene — all elements, their positions, and animation state.',
      inputSchema: { type: 'object', properties: {} },
    },
    {
      name: 'add_element',
      description: 'Add a new element to the 3D scene. Types: box, sphere, text, html-plane, image-plane.',
      inputSchema: {
        type: 'object',
        required: ['type', 'name'],
        properties: {
          type: { type: 'string', enum: ['box', 'sphere', 'text', 'html-plane', 'image-plane'] },
          name: { type: 'string', description: 'Display name for the element' },
          position: { type: 'array', items: { type: 'number' }, minItems: 3, maxItems: 3, description: '[x, y, z]' },
          rotation: { type: 'array', items: { type: 'number' }, minItems: 3, maxItems: 3, description: '[rx, ry, rz] in radians' },
          scale: { type: 'array', items: { type: 'number' }, minItems: 3, maxItems: 3 },
          color: { type: 'string', description: 'Hex color e.g. #ff4488' },
          opacity: { type: 'number', minimum: 0, maximum: 1 },
          width: { type: 'number', description: 'Width in scene units' },
          height: { type: 'number', description: 'Height in scene units' },
          htmlContent: { type: 'string', description: 'HTML string to render as a texture (for html-plane type)' },
          imageUrl: { type: 'string', description: 'Image URL (for image-plane type)' },
          textContent: { type: 'string', description: 'Text to display (for text type)' },
        },
      },
    },
    {
      name: 'update_element',
      description: 'Update properties of an existing scene element by its ID.',
      inputSchema: {
        type: 'object',
        required: ['id'],
        properties: {
          id: { type: 'string' },
          name: { type: 'string' },
          position: { type: 'array', items: { type: 'number' }, minItems: 3, maxItems: 3 },
          rotation: { type: 'array', items: { type: 'number' }, minItems: 3, maxItems: 3 },
          scale: { type: 'array', items: { type: 'number' }, minItems: 3, maxItems: 3 },
          color: { type: 'string' },
          opacity: { type: 'number', minimum: 0, maximum: 1 },
          htmlContent: { type: 'string' },
          textContent: { type: 'string' },
          visible: { type: 'boolean' },
          width: { type: 'number' },
          height: { type: 'number' },
        },
      },
    },
    {
      name: 'remove_element',
      description: 'Remove an element from the scene by its ID.',
      inputSchema: {
        type: 'object',
        required: ['id'],
        properties: { id: { type: 'string' } },
      },
    },
    {
      name: 'select_element',
      description: 'Select an element in the scene (highlights it in the viewport and shows its properties).',
      inputSchema: {
        type: 'object',
        required: ['id'],
        properties: { id: { type: 'string' } },
      },
    },
    {
      name: 'play_animation',
      description: 'Play the animation sequence from the current playhead position.',
      inputSchema: { type: 'object', properties: {} },
    },
    {
      name: 'pause_animation',
      description: 'Pause the animation sequence.',
      inputSchema: { type: 'object', properties: {} },
    },
    {
      name: 'seek_animation',
      description: 'Move the animation playhead to a specific time in seconds.',
      inputSchema: {
        type: 'object',
        required: ['time'],
        properties: { time: { type: 'number', description: 'Time in seconds' } },
      },
    },
    {
      name: 'arrange_demo_layout',
      description: 'Auto-arrange elements into a product demo layout — title, content plane, and supporting visuals in a cinematic composition.',
      inputSchema: {
        type: 'object',
        properties: {
          title: { type: 'string', description: 'Main title text' },
          subtitle: { type: 'string', description: 'Subtitle or tagline' },
          accentColor: { type: 'string', description: 'Brand accent color (hex)' },
          backgroundStyle: { type: 'string', enum: ['dark', 'gradient', 'clean'], description: 'Background style' },
        },
      },
    },
    {
      name: 'create_intro_animation',
      description: 'Create a product intro animation — elements fly in, title reveals, camera moves. Adds keyframes to Theatre.js timeline.',
      inputSchema: {
        type: 'object',
        required: ['elementIds'],
        properties: {
          elementIds: {
            type: 'array',
            items: { type: 'string' },
            description: 'IDs of elements to animate in',
          },
          style: {
            type: 'string',
            enum: ['fade-up', 'slide-in', 'zoom-in', 'spiral'],
            description: 'Animation style',
          },
          duration: { type: 'number', description: 'Duration in seconds' },
        },
      },
    },
  ],
}))

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params
  const a = args as any

  try {
    switch (name) {
      case 'get_scene': {
        const result = await sendToEditor({ type: 'get_scene' })
        return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] }
      }

      case 'add_element': {
        const result = await sendToEditor({ type: 'add_element', payload: a })
        return { content: [{ type: 'text', text: `Added element. ID: ${JSON.stringify(result)}` }] }
      }

      case 'update_element': {
        const { id, ...patch } = a
        await sendToEditor({ type: 'update_element', payload: { id, patch } })
        return { content: [{ type: 'text', text: `Updated element ${id}` }] }
      }

      case 'remove_element': {
        await sendToEditor({ type: 'remove_element', payload: { id: a.id } })
        return { content: [{ type: 'text', text: `Removed element ${a.id}` }] }
      }

      case 'select_element': {
        await sendToEditor({ type: 'select_element', payload: { id: a.id } })
        return { content: [{ type: 'text', text: `Selected ${a.id}` }] }
      }

      case 'play_animation': {
        await sendToEditor({ type: 'play' })
        return { content: [{ type: 'text', text: 'Playing animation' }] }
      }

      case 'pause_animation': {
        await sendToEditor({ type: 'pause' })
        return { content: [{ type: 'text', text: 'Paused' }] }
      }

      case 'seek_animation': {
        await sendToEditor({ type: 'seek', payload: { time: a.time } })
        return { content: [{ type: 'text', text: `Seeked to ${a.time}s` }] }
      }

      case 'arrange_demo_layout': {
        // Creates a cinematic product demo layout
        const color = a.accentColor ?? '#6644ff'
        const title = a.title ?? 'Your Product'
        const subtitle = a.subtitle ?? 'The tagline'

        const titleHtml = `<div style="background:transparent;color:white;font-family:'Inter',sans-serif;text-align:center;padding:16px"><h1 style="font-size:48px;font-weight:800;letter-spacing:-0.03em;margin:0;background:linear-gradient(135deg,${color},#ffffff);-webkit-background-clip:text;-webkit-text-fill-color:transparent">${title}</h1><p style="font-size:20px;color:rgba(255,255,255,0.6);margin-top:8px">${subtitle}</p></div>`

        const cardHtml = `<div style="background:rgba(20,20,30,0.95);border:1px solid ${color}44;border-radius:16px;padding:32px;font-family:'Inter',sans-serif;color:white"><h2 style="color:${color};margin:0 0 16px">Key Features</h2><ul style="margin:0;padding:0;list-style:none;display:flex;flex-direction:column;gap:12px"><li style="display:flex;align-items:center;gap:10px"><span style="color:${color}">▶</span> Feature one here</li><li style="display:flex;align-items:center;gap:10px"><span style="color:${color}">▶</span> Feature two here</li><li style="display:flex;align-items:center;gap:10px"><span style="color:${color}">▶</span> Feature three here</li></ul></div>`

        const bgHtml = `<div style="width:100%;height:100%;background:radial-gradient(ellipse at center,${color}22 0%,#0d0d1a 70%)"></div>`

        // Add all elements via the editor
        const bg = await sendToEditor({ type: 'add_element', payload: { type: 'html-plane', name: 'Background', position: [0, 1.5, -3], rotation: [0, 0, 0], scale: [1, 1, 1], htmlContent: bgHtml, width: 16, height: 9, opacity: 0.8 } })
        const titleEl = await sendToEditor({ type: 'add_element', payload: { type: 'html-plane', name: 'Title', position: [0, 2.5, 0], rotation: [0, 0, 0], scale: [1, 1, 1], htmlContent: titleHtml, width: 6, height: 1.5, opacity: 1 } })
        const card = await sendToEditor({ type: 'add_element', payload: { type: 'html-plane', name: 'Feature Card', position: [0, 0.8, 0], rotation: [0, 0, 0], scale: [1, 1, 1], htmlContent: cardHtml, width: 5, height: 3, opacity: 1 } })

        return {
          content: [{
            type: 'text',
            text: `Created product demo layout with 3 elements. Use the Theatre.js timeline (bottom panel in editor) to add keyframes and animate them. Element IDs: background=${JSON.stringify(bg?.id)}, title=${JSON.stringify(titleEl?.id)}, card=${JSON.stringify(card?.id)}`
          }]
        }
      }

      case 'create_intro_animation': {
        const style = a.style ?? 'fade-up'
        const dur = a.duration ?? 3
        const ids: string[] = a.elementIds ?? []

        return {
          content: [{
            type: 'text',
            text: `To create a "${style}" animation for ${ids.length} elements over ${dur}s:\n\n1. The Theatre.js Studio panel at the bottom of the editor is your keyframe editor\n2. Select each element in the outliner\n3. In the Studio panel, set your start position/opacity at t=0, then advance the playhead to t=${dur} and set the final values\n4. Theatre.js will interpolate automatically\n\nFor a ${style} animation: start with opacity=0 and position offset, end with opacity=1 at final position. Click the diamond icon (◆) next to any property in the Studio panel to add a keyframe.\n\nElement IDs to animate: ${ids.join(', ')}`
          }]
        }
      }

      default:
        return { content: [{ type: 'text', text: `Unknown tool: ${name}` }], isError: true }
    }
  } catch (err: any) {
    return {
      content: [{ type: 'text', text: `Error: ${err.message}` }],
      isError: true,
    }
  }
})

// ── Start ─────────────────────────────────────────────────────────────────────
const transport = new StdioServerTransport()
await server.connect(transport)
console.error('[Juicer MCP] Server ready (stdio)')
