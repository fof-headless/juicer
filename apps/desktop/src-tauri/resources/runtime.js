// Juicer renderer runtime.
//
// Lives inside renderer.html. Exposes `window.__juicer.renderFrame(payload)`
// which the Swift helper (juicer-frame-renderer) calls via callAsyncJavaScript
// for every frame, and which the Tauri UI also calls via postMessage for live
// preview.
//
// payload = {
//   scene_doc:    { layers: [{id, kind, html_srcdoc?, image_src?, text_content?, shape?}], background },
//   state:        { layers: [{id, visible, style: {...css-prop: value}, text?}], canvas_width, canvas_height },
//   canvas_width: number,
//   canvas_height: number
// }
//
// The function:
//   1) Resizes the body/stage to canvas dimensions.
//   2) Rebuilds the layer DOM if scene_doc differs from the cached structure (cheap diff by id+kind).
//   3) Applies per-layer styles + (for text layers) text content from state.
//   4) Waits for two requestAnimationFrames so layout AND compositing have run.
//   5) Resolves the returned Promise — Swift then snapshots.

(function () {
  const stage = document.getElementById('stage');
  const cached = {
    background: null,
    layerStructure: null,  // JSON string of [{id, kind, content_hash}]
    canvasWidth: 0,
    canvasHeight: 0,
    layerEls: new Map(),   // id -> { wrapper, inner }
  };

  function structureFingerprint(scene_doc) {
    return JSON.stringify(scene_doc.layers.map(l => ({
      id: l.id,
      kind: l.kind,
      // Fingerprint the content so we know when to rebuild the inner element.
      h: hashString(
        l.html_srcdoc || l.image_src || l.text_content || l.shape || ''
      ),
    })));
  }

  function hashString(s) {
    // Tiny FNV-1a so we don't rebuild for unchanged content.
    let h = 0x811c9dc5;
    for (let i = 0; i < s.length; i++) {
      h ^= s.charCodeAt(i);
      h = (h + ((h << 1) + (h << 4) + (h << 7) + (h << 8) + (h << 24))) >>> 0;
    }
    return h.toString(16);
  }

  function buildLayer(l) {
    const wrapper = document.createElement('div');
    wrapper.className = 'juicer-layer';
    wrapper.dataset.id = l.id;
    wrapper.dataset.kind = l.kind;

    if (l.kind === 'html') {
      const iframe = document.createElement('iframe');
      iframe.srcdoc = l.html_srcdoc || '';
      // sandbox keeps the user HTML from navigating/exec'ing top-level things;
      // allow-same-origin so fonts/Tailwind load. NO allow-scripts unless the
      // user explicitly wants animated HTML — for v1, scripts in srcdoc are off.
      iframe.setAttribute('sandbox', 'allow-same-origin allow-scripts');
      wrapper.appendChild(iframe);
    } else if (l.kind === 'image') {
      const img = document.createElement('img');
      img.className = 'juicer-img';
      img.src = l.image_src || '';
      wrapper.appendChild(img);
    } else if (l.kind === 'text') {
      // The text node lives directly in the wrapper; per-frame styles set font/color.
      wrapper.textContent = l.text_content || '';
    } else if (l.kind === 'shape') {
      // Shape is just the wrapper with background/border styles applied via state.style.
      // For ellipse, the resolved style includes border-radius:50%.
      wrapper.dataset.shape = l.shape || 'rect';
    }
    return wrapper;
  }

  function rebuild(scene_doc) {
    stage.innerHTML = '';
    cached.layerEls.clear();
    for (const l of scene_doc.layers) {
      const wrapper = buildLayer(l);
      stage.appendChild(wrapper);
      cached.layerEls.set(l.id, wrapper);
    }
  }

  function applyState(state) {
    for (const ls of state.layers || []) {
      const el = cached.layerEls.get(ls.id);
      if (!el) continue;
      el.style.display = ls.visible === false ? 'none' : '';
      const styles = ls.style || {};
      for (const [k, v] of Object.entries(styles)) {
        // setProperty handles kebab-case keys; '' clears.
        try { el.style.setProperty(k, v); } catch (e) {}
      }
      if (ls.text != null && el.dataset.kind === 'text') {
        if (el.textContent !== ls.text) el.textContent = ls.text;
      }
    }
  }

  function waitTwoFrames() {
    // Offscreen WKWebViews don't always fire requestAnimationFrame (no display
    // refresh tied to them). Use setTimeout for reliable yield-and-resume so
    // layout/style recalc has happened before we tell Swift to snapshot.
    return new Promise(resolve => {
      setTimeout(() => setTimeout(resolve, 0), 16);
    });
  }

  async function renderFrame(payload) {
    const { scene_doc, state, canvas_width, canvas_height } = payload || {};
    if (!scene_doc || !state) {
      throw new Error('renderFrame: missing scene_doc or state');
    }

    // Canvas size.
    if (canvas_width !== cached.canvasWidth || canvas_height !== cached.canvasHeight) {
      stage.style.width = canvas_width + 'px';
      stage.style.height = canvas_height + 'px';
      // Body fills the WebView; #stage is the canvas-sized rectangle within.
      document.body.style.width = canvas_width + 'px';
      document.body.style.height = canvas_height + 'px';
      cached.canvasWidth = canvas_width;
      cached.canvasHeight = canvas_height;
    }

    // Background.
    if (scene_doc.background !== cached.background) {
      stage.style.background = scene_doc.background || 'transparent';
      cached.background = scene_doc.background;
    }

    // Diff structure; rebuild iff layer ids/kinds/content changed.
    const fp = structureFingerprint(scene_doc);
    if (fp !== cached.layerStructure) {
      rebuild(scene_doc);
      cached.layerStructure = fp;
    }

    // Apply per-layer styles + text overrides.
    applyState(state);

    // Wait for layout + compositing.
    await waitTwoFrames();

    return true;
  }

  window.__juicer = {
    renderFrame,
    // Direct hooks for in-process callers (the Tauri UI iframe) — postMessage
    // path also forwards to renderFrame.
  };

  // Live-preview path: the Tauri React UI postMessages frames into this iframe.
  window.addEventListener('message', async (e) => {
    const d = e.data;
    if (!d || d.kind !== 'juicer-render') return;
    try {
      await renderFrame(d.payload);
      // Echo so the parent can know we're ready for the next frame.
      e.source && e.source.postMessage({ kind: 'juicer-rendered', frame: d.payload.state?.frame ?? 0 }, '*');
    } catch (err) {
      e.source && e.source.postMessage({ kind: 'juicer-error', error: String(err) }, '*');
    }
  });
})();
