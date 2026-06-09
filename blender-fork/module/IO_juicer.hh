/* SPDX-FileCopyrightText: 2026 Juicer
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

/** \file
 * \ingroup juicer
 *
 * Juicer scene importer/renderer — the native (no-Python) entry point that
 * lets the Blender binary ingest a Juicer scene.json directly and render it.
 *
 * Invoked from creator_args.cc via the `--juicer <scene.json>` CLI flag.
 */

#pragma once

/* In Blender 5.x bContext is forward-declared inside `namespace blender`
 * (see BKE_context.hh), so we must match that here — a global `struct
 * bContext;` would create a distinct ::bContext type and break linking. */
namespace blender {
struct bContext;
}

namespace blender::io::juicer {

/**
 * Build a Blender scene from a Juicer scene.json and render it.
 *
 * - Parses the JSON (elements, camera, render settings, keyframe tracks).
 * - Creates plane/box/sphere objects, textures planes with captured HTML PNGs.
 * - Inserts real F-curve keyframes (position/rotation/scale/opacity, camera).
 * - Configures render resolution/fps/frame-range and renders to `out_path`.
 *
 * \param json_path: absolute path to the Juicer scene.json.
 * \param out_path: output path; an image sequence dir or an .mp4 (FFMPEG).
 * \param single_frame: if >= 0, render only that frame; else the full range.
 * \return true on success.
 */
bool import_and_render(bContext *C,
                       const char *json_path,
                       const char *out_path,
                       int single_frame);

}  // namespace blender::io::juicer
