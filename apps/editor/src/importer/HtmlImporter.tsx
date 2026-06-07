import { useState, useCallback } from 'react'
import { useDropzone } from 'react-dropzone'
import { useSceneStore } from '../store/scene'
import { Upload, Code, Image as ImageIcon, Type, Box, Circle } from 'lucide-react'
import './HtmlImporter.css'

export function HtmlImporter() {
  const addElement = useSceneStore((s) => s.addElement)
  const [htmlInput, setHtmlInput] = useState('')
  const [tab, setTab] = useState<'quick-add' | 'html' | 'image'>('quick-add')

  const onDrop = useCallback(
    (files: File[]) => {
      files.forEach((file) => {
        if (file.type.startsWith('image/')) {
          const url = URL.createObjectURL(file)
          addElement({
            name: file.name,
            type: 'image-plane',
            visible: true,
            locked: false,
            position: [0, 0, 0],
            rotation: [0, 0, 0],
            scale: [1, 1, 1],
            imageUrl: url,
            width: 4,
            height: 2.5,
            opacity: 1,
          })
        } else if (file.type === 'text/html' || file.name.endsWith('.html')) {
          file.text().then((html) => {
            addElement({
              name: file.name.replace('.html', ''),
              type: 'html-plane',
              visible: true,
              locked: false,
              position: [0, 0, 0],
              rotation: [0, 0, 0],
              scale: [1, 1, 1],
              htmlContent: html,
              width: 4,
              height: 2.5,
              opacity: 1,
            })
          })
        }
      })
    },
    [addElement]
  )

  const { getRootProps, getInputProps, isDragActive } = useDropzone({
    onDrop,
    noClick: true,
    accept: { 'image/*': [], 'text/html': ['.html'] },
  })

  const addHtmlPlane = () => {
    if (!htmlInput.trim()) return
    addElement({
      name: `HTML Block ${Date.now()}`,
      type: 'html-plane',
      visible: true,
      locked: false,
      position: [0, 1, 0],
      rotation: [0, 0, 0],
      scale: [1, 1, 1],
      htmlContent: htmlInput,
      width: 4,
      height: 2.5,
      opacity: 1,
      color: '#1a1a2e',
    })
    setHtmlInput('')
  }

  const quickAdd = (type: 'box' | 'sphere' | 'text', label: string) => {
    addElement({
      name: label,
      type,
      visible: true,
      locked: false,
      position: [Math.random() * 2 - 1, 0.5, Math.random() * 2 - 1],
      rotation: [0, 0, 0],
      scale: [1, 1, 1],
      textContent: type === 'text' ? 'Your Text Here' : undefined,
      color: type === 'box' ? '#4488ff' : type === 'sphere' ? '#ff4488' : '#ffffff',
      opacity: 1,
      width: 1,
      height: 1,
    })
  }

  return (
    <div className="importer" {...getRootProps()}>
      <input {...getInputProps()} />
      {isDragActive && (
        <div className="importer-drop-overlay">
          <Upload size={32} />
          <span>Drop image or HTML file</span>
        </div>
      )}

      <div className="importer-tabs">
        {(['quick-add', 'html', 'image'] as const).map((t) => (
          <button key={t} className={`importer-tab ${tab === t ? 'active' : ''}`} onClick={() => setTab(t)}>
            {t === 'quick-add' ? 'Add' : t === 'html' ? 'HTML' : 'Image'}
          </button>
        ))}
      </div>

      {tab === 'quick-add' && (
        <div className="importer-quickadd">
          <button className="qa-btn" onClick={() => quickAdd('box', 'Box')}>
            <Box size={16} /> Box
          </button>
          <button className="qa-btn" onClick={() => quickAdd('sphere', 'Sphere')}>
            <Circle size={16} /> Sphere
          </button>
          <button className="qa-btn" onClick={() => quickAdd('text', 'Text')}>
            <Type size={16} /> Text
          </button>
          <button
            className="qa-btn"
            onClick={() =>
              addElement({
                name: 'HTML Plane',
                type: 'html-plane',
                visible: true,
                locked: false,
                position: [0, 1, 0],
                rotation: [0, 0, 0],
                scale: [1, 1, 1],
                htmlContent: '<div style="background:#1a1a3e;color:white;padding:20px;font-family:sans-serif;border-radius:12px;border:1px solid #6644ff"><h2>Your Brand Component</h2><p>Paste your HTML in Properties panel</p></div>',
                width: 4,
                height: 2.5,
                opacity: 1,
              })
            }
          >
            <Code size={16} /> HTML Plane
          </button>
          <button
            className="qa-btn"
            onClick={() =>
              addElement({
                name: 'Image',
                type: 'image-plane',
                visible: true,
                locked: false,
                position: [0, 1, 0],
                rotation: [0, 0, 0],
                scale: [1, 1, 1],
                width: 4,
                height: 2.5,
                opacity: 1,
              })
            }
          >
            <ImageIcon size={16} /> Image
          </button>
          <div className="qa-hint">
            <Upload size={12} />
            Drop images or .html files anywhere
          </div>
        </div>
      )}

      {tab === 'html' && (
        <div className="importer-html">
          <p className="importer-hint">Paste your HTML/React component output here. It will be rendered as a texture on a 3D plane.</p>
          <textarea
            className="importer-textarea"
            value={htmlInput}
            onChange={(e) => setHtmlInput(e.target.value)}
            placeholder={`<div style="background:#1a1a3e;color:white;padding:24px;border-radius:12px">\n  <h1>Your Brand</h1>\n  <p>Product demo content</p>\n</div>`}
            rows={10}
          />
          <button className="importer-btn" onClick={addHtmlPlane} disabled={!htmlInput.trim()}>
            <Code size={14} /> Add to Scene
          </button>
        </div>
      )}

      {tab === 'image' && (
        <div className="importer-image">
          <div className="importer-dropzone">
            <Upload size={24} />
            <span>Drop image here or click</span>
            <input
              type="file"
              accept="image/*"
              style={{ position: 'absolute', inset: 0, opacity: 0, cursor: 'pointer' }}
              onChange={(e) => {
                const file = e.target.files?.[0]
                if (!file) return
                const url = URL.createObjectURL(file)
                addElement({
                  name: file.name,
                  type: 'image-plane',
                  visible: true,
                  locked: false,
                  position: [0, 1, 0],
                  rotation: [0, 0, 0],
                  scale: [1, 1, 1],
                  imageUrl: url,
                  width: 4,
                  height: 2.5,
                  opacity: 1,
                })
              }}
            />
          </div>
        </div>
      )}
    </div>
  )
}
