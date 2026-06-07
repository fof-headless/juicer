"""
Juicer Blender Bridge
---------------------
Run inside Blender headless:
    blender --background --python juicer_bridge.py

Opens a TCP server on 127.0.0.1:6789.
Receives newline-delimited JSON commands from the Tauri app.
Executes them in Blender and returns JSON responses.

Blender version: 4.x+ (uses bpy, bmesh, mathutils)
"""

import bpy
import bmesh
import mathutils
import json
import socket
import threading
import traceback
import os
import sys
import struct
from pathlib import Path

HOST = "127.0.0.1"
PORT = 6789


# ── Helpers ───────────────────────────────────────────────────────────────────

def hex_to_rgb(hex_color: str) -> tuple:
    """Convert #rrggbb to (r, g, b, 1.0) linear float."""
    hex_color = hex_color.lstrip("#")
    r, g, b = (int(hex_color[i:i+2], 16) / 255 for i in (0, 2, 4))
    # sRGB → linear (approximate gamma 2.2)
    return (pow(r, 2.2), pow(g, 2.2), pow(b, 2.2), 1.0)


def ensure_material(obj, color_hex: str):
    """Create or replace a simple emission material on obj."""
    mat = bpy.data.materials.new(name=f"JMat_{obj.name}")
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    nodes.clear()

    bsdf = nodes.new("ShaderNodeBsdfPrincipled")
    bsdf.inputs["Base Color"].default_value = hex_to_rgb(color_hex)
    bsdf.inputs["Roughness"].default_value = 0.5
    bsdf.inputs["Metallic"].default_value = 0.0

    out = nodes.new("ShaderNodeOutputMaterial")
    mat.node_tree.links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])

    if obj.data.materials:
        obj.data.materials[0] = mat
    else:
        obj.data.materials.append(mat)
    return mat


def ensure_image_material(obj, image_path: str):
    """Apply an image texture material to a plane."""
    mat = bpy.data.materials.new(name=f"JImg_{obj.name}")
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links
    nodes.clear()

    tex_node = nodes.new("ShaderNodeTexImage")
    img = bpy.data.images.load(image_path)
    img.colorspace_settings.name = "sRGB"
    tex_node.image = img

    bsdf = nodes.new("ShaderNodeBsdfPrincipled")
    bsdf.inputs["Roughness"].default_value = 1.0
    bsdf.inputs["Specular IOR Level"].default_value = 0.0
    links.new(tex_node.outputs["Color"], bsdf.inputs["Base Color"])
    links.new(tex_node.outputs["Alpha"], bsdf.inputs["Alpha"])
    mat.blend_method = "BLEND"

    out = nodes.new("ShaderNodeOutputMaterial")
    links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])

    if obj.data.materials:
        obj.data.materials[0] = mat
    else:
        obj.data.materials.append(mat)


def set_obj_transform(obj, location=None, rotation=None, scale=None):
    if location:
        obj.location = mathutils.Vector(location)
    if rotation:
        obj.rotation_euler = mathutils.Euler(rotation, "XYZ")
    if scale:
        obj.scale = mathutils.Vector(scale)


def scene_object_to_dict(obj) -> dict:
    return {
        "name": obj.name,
        "type": obj.type,
        "location": list(obj.location),
        "rotation": [obj.rotation_euler.x, obj.rotation_euler.y, obj.rotation_euler.z],
        "scale": list(obj.scale),
        "visible": not obj.hide_viewport,
        "keyframes": get_keyframe_summary(obj),
    }


def get_keyframe_summary(obj) -> list:
    keyframes = []
    if obj.animation_data and obj.animation_data.action:
        for fc in obj.animation_data.action.fcurves:
            for kp in fc.keyframe_points:
                keyframes.append({
                    "frame": kp.co[0],
                    "data_path": fc.data_path,
                    "array_index": fc.array_index,
                    "value": kp.co[1],
                })
    return keyframes


# ── Command handlers ──────────────────────────────────────────────────────────

def handle_ping(_):
    return {"ok": True, "version": bpy.app.version_string}


def handle_get_scene(_):
    return {
        "objects": [scene_object_to_dict(o) for o in bpy.context.scene.objects],
        "frame_current": bpy.context.scene.frame_current,
        "frame_start": bpy.context.scene.frame_start,
        "frame_end": bpy.context.scene.frame_end,
        "fps": bpy.context.scene.render.fps,
        "render_resolution": [
            bpy.context.scene.render.resolution_x,
            bpy.context.scene.render.resolution_y,
        ],
    }


def handle_add_object(cmd):
    kind = cmd.get("kind", "box")
    name = cmd.get("name", "Object")
    location = cmd.get("location", [0, 0, 0])
    rotation = cmd.get("rotation", [0, 0, 0])
    scale = cmd.get("scale", [1, 1, 1])
    color = cmd.get("color", "#4488ff")
    image_path = cmd.get("image_path")
    text_str = cmd.get("text", "")
    width = float(cmd.get("width", 2.0))
    height = float(cmd.get("height", 2.0))

    bpy.ops.object.select_all(action="DESELECT")

    if kind in ("box", "cube"):
        bpy.ops.mesh.primitive_cube_add(size=1, location=location)
    elif kind == "sphere":
        bpy.ops.mesh.primitive_uv_sphere_add(radius=0.5, location=location, segments=32, ring_count=16)
    elif kind in ("plane", "html-plane", "image-plane"):
        bpy.ops.mesh.primitive_plane_add(size=1, location=location)
        # Scale to width/height
        obj = bpy.context.active_object
        obj.scale = (width / 2, height / 2, 1)
    elif kind == "cylinder":
        bpy.ops.mesh.primitive_cylinder_add(radius=0.5, depth=1.0, location=location)
    elif kind == "text":
        bpy.ops.object.text_add(location=location)
        obj = bpy.context.active_object
        obj.data.body = text_str
        obj.data.align_x = "CENTER"
        obj.data.align_y = "CENTER"
        obj.data.size = 0.4
        # Extrude slightly for 3D look
        obj.data.extrude = 0.02
        obj.name = name
        set_obj_transform(obj, rotation=rotation, scale=scale)
        ensure_material(obj, color)
        return scene_object_to_dict(obj)
    elif kind == "light":
        bpy.ops.object.light_add(type="AREA", location=location)
        obj = bpy.context.active_object
        obj.data.energy = float(cmd.get("energy", 500))
        obj.name = name
        set_obj_transform(obj, rotation=rotation)
        return scene_object_to_dict(obj)
    else:
        return {"error": f"Unknown kind: {kind}"}

    obj = bpy.context.active_object
    obj.name = name
    set_obj_transform(obj, rotation=rotation)

    # Don't overwrite scale if we already set it (plane case)
    if kind not in ("plane", "html-plane", "image-plane"):
        obj.scale = mathutils.Vector(scale)

    if image_path and os.path.exists(image_path):
        ensure_image_material(obj, image_path)
    elif kind not in ("text", "light"):
        ensure_material(obj, color)

    # Smooth shading for curved objects
    if kind in ("sphere", "cylinder"):
        bpy.ops.object.shade_smooth()

    return scene_object_to_dict(obj)


def handle_update_object(cmd):
    name = cmd.get("name")
    obj = bpy.data.objects.get(name)
    if not obj:
        return {"error": f"Object '{name}' not found"}

    if "location" in cmd:
        obj.location = mathutils.Vector(cmd["location"])
    if "rotation" in cmd:
        obj.rotation_euler = mathutils.Euler(cmd["rotation"], "XYZ")
    if "scale" in cmd:
        obj.scale = mathutils.Vector(cmd["scale"])
    if "color" in cmd and obj.data.materials:
        mat = obj.data.materials[0]
        if mat and mat.use_nodes:
            for node in mat.node_tree.nodes:
                if node.type == "BSDF_PRINCIPLED":
                    node.inputs["Base Color"].default_value = hex_to_rgb(cmd["color"])
    if "visible" in cmd:
        obj.hide_viewport = not cmd["visible"]
        obj.hide_render = not cmd["visible"]
    if "image_path" in cmd and cmd["image_path"]:
        ensure_image_material(obj, cmd["image_path"])

    return scene_object_to_dict(obj)


def handle_remove_object(cmd):
    name = cmd.get("name")
    obj = bpy.data.objects.get(name)
    if not obj:
        return {"error": f"Object '{name}' not found"}
    bpy.data.objects.remove(obj, do_unlink=True)
    return {"removed": name}


def handle_set_keyframe(cmd):
    name = cmd.get("name")
    frame = int(cmd.get("frame", 1))
    prop = cmd.get("property", "location")  # location | rotation_euler | scale | color | alpha
    value = cmd.get("value")

    obj = bpy.data.objects.get(name)
    if not obj:
        return {"error": f"Object '{name}' not found"}

    bpy.context.scene.frame_set(frame)

    if prop == "location" and value:
        obj.location = mathutils.Vector(value)
        obj.keyframe_insert(data_path="location", frame=frame)
    elif prop == "rotation_euler" and value:
        obj.rotation_euler = mathutils.Euler(value, "XYZ")
        obj.keyframe_insert(data_path="rotation_euler", frame=frame)
    elif prop == "scale" and value:
        obj.scale = mathutils.Vector(value)
        obj.keyframe_insert(data_path="scale", frame=frame)
    elif prop == "alpha" and obj.data.materials:
        mat = obj.data.materials[0]
        if mat and mat.use_nodes:
            for node in mat.node_tree.nodes:
                if node.type == "BSDF_PRINCIPLED":
                    node.inputs["Alpha"].default_value = float(value)
                    node.inputs["Alpha"].keyframe_insert("default_value", frame=frame)

    return {"keyframed": name, "frame": frame, "property": prop}


def handle_seek(cmd):
    frame = int(cmd.get("frame", 1))
    bpy.context.scene.frame_set(frame)
    return {"frame": frame}


def handle_render_frame(cmd):
    frame = int(cmd.get("frame", bpy.context.scene.frame_current))
    output_path = cmd.get("output_path", "/tmp/juicer_render.png")

    bpy.context.scene.frame_set(frame)
    bpy.context.scene.render.engine = "BLENDER_EEVEE_NEXT"
    bpy.context.scene.render.image_settings.file_format = "PNG"
    bpy.context.scene.render.filepath = output_path

    bpy.ops.render.render(write_still=True)
    return {"rendered": output_path, "frame": frame}


def handle_render_animation(cmd):
    output_path = cmd.get("output_path", "/tmp/juicer_")
    start = int(cmd.get("start", bpy.context.scene.frame_start))
    end = int(cmd.get("end", bpy.context.scene.frame_end))
    fps = int(cmd.get("fps", 30))
    fmt = cmd.get("format", "FFMPEG")

    scene = bpy.context.scene
    scene.frame_start = start
    scene.frame_end = end
    scene.render.fps = fps
    scene.render.engine = "BLENDER_EEVEE_NEXT"
    scene.render.filepath = output_path

    if fmt == "FFMPEG":
        scene.render.image_settings.file_format = "FFMPEG"
        scene.render.ffmpeg.format = "MPEG4"
        scene.render.ffmpeg.codec = "H264"
        scene.render.ffmpeg.constant_rate_factor = "MEDIUM"
        scene.render.ffmpeg.audio_codec = "NONE"
    else:
        scene.render.image_settings.file_format = "PNG"

    bpy.ops.render.render(animation=True)
    return {"rendered": output_path, "start": start, "end": end, "fps": fps}


def handle_set_fps(cmd):
    bpy.context.scene.render.fps = int(cmd.get("fps", 30))
    return {"fps": bpy.context.scene.render.fps}


def handle_set_resolution(cmd):
    bpy.context.scene.render.resolution_x = int(cmd.get("width", 1920))
    bpy.context.scene.render.resolution_y = int(cmd.get("height", 1080))
    bpy.context.scene.render.resolution_percentage = 100
    return {"width": cmd["width"], "height": cmd["height"]}


def handle_set_background(cmd):
    color = cmd.get("color", [0.05, 0.05, 0.1, 1.0])
    world = bpy.context.scene.world
    if not world:
        world = bpy.data.worlds.new("JuicerWorld")
        bpy.context.scene.world = world
    world.use_nodes = True
    bg_node = world.node_tree.nodes.get("Background")
    if bg_node:
        bg_node.inputs["Color"].default_value = color
        bg_node.inputs["Strength"].default_value = 1.0
    return {"background_color": color}


HANDLERS = {
    "ping": handle_ping,
    "get_scene": handle_get_scene,
    "add_object": handle_add_object,
    "update_object": handle_update_object,
    "remove_object": handle_remove_object,
    "set_keyframe": handle_set_keyframe,
    "seek": handle_seek,
    "render_frame": handle_render_frame,
    "render_animation": handle_render_animation,
    "set_fps": handle_set_fps,
    "set_resolution": handle_set_resolution,
    "set_background": handle_set_background,
}


# ── Initial scene setup ───────────────────────────────────────────────────────

def setup_scene():
    """Prepare a clean scene with Eevee and good defaults."""
    # Clear default objects (cube, light, camera)
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete()

    scene = bpy.context.scene
    scene.render.engine = "BLENDER_EEVEE_NEXT"
    scene.render.resolution_x = 1920
    scene.render.resolution_y = 1080
    scene.render.fps = 30
    scene.frame_start = 1
    scene.frame_end = 300  # 10s at 30fps

    # Eevee settings for quality
    eevee = scene.eevee
    eevee.use_bloom = True
    eevee.bloom_intensity = 0.05
    eevee.use_ssr = True

    # Camera
    bpy.ops.object.camera_add(location=(0, -6, 2), rotation=(1.2, 0, 0))
    cam = bpy.context.active_object
    cam.name = "JuicerCamera"
    scene.camera = cam

    # Key light
    bpy.ops.object.light_add(type="AREA", location=(3, -3, 5))
    key = bpy.context.active_object
    key.name = "KeyLight"
    key.data.energy = 800
    key.data.size = 3
    key.rotation_euler = (0.8, 0.2, 0.6)

    # Fill light
    bpy.ops.object.light_add(type="AREA", location=(-3, -2, 3))
    fill = bpy.context.active_object
    fill.name = "FillLight"
    fill.data.energy = 200
    fill.data.size = 5

    # Rim light (back light for depth)
    bpy.ops.object.light_add(type="SPOT", location=(0, 4, 4))
    rim = bpy.context.active_object
    rim.name = "RimLight"
    rim.data.energy = 300
    rim.rotation_euler = (-0.8, 0, 0)

    # Dark world
    handle_set_background({"color": [0.03, 0.03, 0.06, 1.0]})

    print("[Juicer] Scene ready — Eevee renderer, 1920×1080, 30fps")


# ── TCP server ────────────────────────────────────────────────────────────────

def handle_client(conn, addr):
    print(f"[Juicer] Client connected: {addr}")
    try:
        buf = b""
        while True:
            chunk = conn.recv(4096)
            if not chunk:
                break
            buf += chunk
            while b"\n" in buf:
                line, buf = buf.split(b"\n", 1)
                line = line.strip()
                if not line:
                    continue
                try:
                    cmd = json.loads(line)
                    op = cmd.get("op", "")
                    handler = HANDLERS.get(op)
                    if handler:
                        result = handler(cmd)
                    else:
                        result = {"error": f"Unknown op: {op}"}
                except Exception:
                    result = {"error": traceback.format_exc()}

                response = json.dumps(result) + "\n"
                conn.sendall(response.encode())
    except Exception as e:
        print(f"[Juicer] Client error: {e}")
    finally:
        conn.close()
        print(f"[Juicer] Client disconnected: {addr}")


def run_server():
    server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind((HOST, PORT))
    server.listen(5)
    print(f"[Juicer] Bridge listening on {HOST}:{PORT}")

    while True:
        conn, addr = server.accept()
        t = threading.Thread(target=handle_client, args=(conn, addr), daemon=True)
        t.start()


# ── Entry point ───────────────────────────────────────────────────────────────

setup_scene()

server_thread = threading.Thread(target=run_server, daemon=True)
server_thread.start()

print("[Juicer] Bridge started. Blender is ready.")

# Keep the Blender process alive
import time
while True:
    time.sleep(1)
