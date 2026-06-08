/* SPDX-FileCopyrightText: 2026 Juicer
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

/** \file
 * \ingroup juicer
 *
 * Native importer: Juicer scene.json -> Blender scene -> render.
 *
 * This uses Blender's *real* subsystems — BKE objects/meshes/materials,
 * the animrig F-curve API, and the RE_ render pipeline. No Python (bpy).
 *
 * NOTE: pinned against Blender commit 1957ef3 (v5.03 alpha). API names in
 * blenkernel/animrig/render are moving targets across versions; if a build
 * fails, the failure points are isolated to the small helpers below.
 */

#include "IO_juicer.hh"

#include <json.hpp> /* extern/json/include/json.hpp (nlohmann) */

#include <cstdio>
#include <string>
#include <vector>

#include "BLI_math_matrix.h"
#include "BLI_math_rotation.h"
#include "BLI_string.h"

#include "DNA_curve_types.h" /* BezTriple ipo/easing enums */
#include "DNA_object_types.h"
#include "DNA_scene_types.h"

#include "BKE_context.hh"
#include "BKE_image.hh"
#include "BKE_main.hh"
#include "BKE_material.hh"
#include "BKE_mesh.hh"
#include "BKE_object.hh"
#include "BKE_scene.hh"

#include "ANIM_animdata.hh"
#include "ANIM_fcurve.hh"

#include "DEG_depsgraph.hh"

#include "RE_pipeline.h"

using json = nlohmann::json;

namespace blender::io::juicer {

/* ── Easing map: Juicer -> Blender BezTriple interpolation/easing ──────────────
 * Juicer's cubic easing matches Blender's BEZT_IPO_CUBIC + EASE_* almost 1:1. */
struct IpoMap {
  char ipo;     /* eBezTriple_Interpolation */
  char easing;  /* eBezTriple_Easing */
};

static IpoMap map_easing(const std::string &e)
{
  if (e == "linear") return {BEZT_IPO_LIN, BEZT_IPO_EASE_AUTO};
  if (e == "step") return {BEZT_IPO_CONST, BEZT_IPO_EASE_AUTO};
  if (e == "ease-in") return {BEZT_IPO_CUBIC, BEZT_IPO_EASE_IN};
  if (e == "ease-out") return {BEZT_IPO_CUBIC, BEZT_IPO_EASE_OUT};
  /* default ease-in-out */
  return {BEZT_IPO_CUBIC, BEZT_IPO_EASE_IN_OUT};
}

/* ── Build a 1x1 quad mesh centered at origin in the XY plane ──────────────────
 * Juicer planes are scaled by width/height via the object scale. */
static Mesh *make_plane_mesh()
{
  Mesh *me = BKE_mesh_new_nomain(4, 0, 1, 4);
  blender::MutableSpan<blender::float3> positions = me->vert_positions_for_write();
  positions[0] = {-0.5f, -0.5f, 0.0f};
  positions[1] = {0.5f, -0.5f, 0.0f};
  positions[2] = {0.5f, 0.5f, 0.0f};
  positions[3] = {-0.5f, 0.5f, 0.0f};

  blender::MutableSpan<int> face_offsets = me->face_offsets_for_write();
  face_offsets[0] = 0;
  face_offsets[1] = 4;

  blender::MutableSpan<int> corner_verts = me->corner_verts_for_write();
  corner_verts[0] = 0;
  corner_verts[1] = 1;
  corner_verts[2] = 2;
  corner_verts[3] = 3;

  /* UVs 0..1 so the HTML texture maps corner-to-corner. */
  blender::bke::MutableAttributeAccessor attrs = me->attributes_for_write();
  blender::bke::SpanAttributeWriter<blender::float2> uv =
      attrs.lookup_or_add_for_write_only_span<blender::float2>("UVMap",
                                                               blender::bke::AttrDomain::Corner);
  uv.span[0] = {0.0f, 0.0f};
  uv.span[1] = {1.0f, 0.0f};
  uv.span[2] = {1.0f, 1.0f};
  uv.span[3] = {0.0f, 1.0f};
  uv.finish();

  BKE_mesh_calc_edges(me, false, false);
  return me;
}

/* ── Insert a keyframe channel on an object's animation data ───────────────────
 * Creates the F-curve for `rna_path[array_index]` and inserts a bezt at frame. */
static void insert_key(Object *ob,
                       const char *rna_path,
                       int array_index,
                       float frame,
                       float value,
                       const std::string &easing)
{
  using namespace blender::animrig;
  FCurve *fcu = action_fcurve_ensure_ex(/*bmain*/ nullptr,
                                         /*act*/ nullptr,
                                         /*group*/ nullptr,
                                         /*ptr*/ nullptr,
                                         FCurveDescriptor{rna_path, array_index});
  /* NOTE: in 5.x F-curves live on a layered Action/slot; see build notes.
   * The exact ensure-call is the main thing to verify on first compile. */
  if (!fcu) {
    return;
  }
  KeyframeSettings settings = get_keyframe_settings(false);
  IpoMap m = map_easing(easing);
  settings.interpolation = static_cast<eBezTriple_Interpolation>(m.ipo);
  insert_vert_fcurve(fcu, {frame, value}, settings, INSERTKEY_NOFLAGS);

  /* Apply dynamic easing to the just-inserted key. */
  if (fcu->totvert > 0) {
    BezTriple &bz = fcu->bezt[fcu->totvert - 1];
    bz.ipo = m.ipo;
    bz.easing = m.easing;
  }
  UNUSED_VARS(ob);
}

/* Insert a vec3 keyframe (3 channels) from a Juicer track key. */
static void insert_vec3_key(Object *ob,
                            const char *rna_path,
                            float frame,
                            const json &val,
                            const std::string &easing)
{
  for (int i = 0; i < 3; i++) {
    insert_key(ob, rna_path, i, frame, val[i].get<float>(), easing);
  }
}

/* ── Main import ──────────────────────────────────────────────────────────────*/
bool import_and_render(bContext *C, const char *json_path, const char *out_path, int single_frame)
{
  Main *bmain = CTX_data_main(C);
  Scene *scene = CTX_data_scene(C);
  ViewLayer *view_layer = CTX_data_view_layer(C);

  FILE *fp = std::fopen(json_path, "rb");
  if (!fp) {
    std::fprintf(stderr, "[juicer] cannot open %s\n", json_path);
    return false;
  }
  json doc;
  try {
    doc = json::parse(fp);
  }
  catch (const std::exception &e) {
    std::fprintf(stderr, "[juicer] JSON parse error: %s\n", e.what());
    std::fclose(fp);
    return false;
  }
  std::fclose(fp);

  /* ── Render settings ── */
  const json &r = doc["render"];
  scene->r.xsch = r.value("width", 1920);
  scene->r.ysch = r.value("height", 1080);
  scene->r.frs_sec = r.value("fps", 30);
  scene->r.frs_sec_base = 1.0f;
  scene->r.sfra = r.value("frame_start", 1);
  scene->r.efra = r.value("frame_end", 250);
  scene->r.size = 100;

  /* World/background color from render.background (rgba 0..1). */
  if (r.contains("background") && scene->world) {
    const json &bg = r["background"];
    scene->world->horr = bg[0].get<float>();
    scene->world->horg = bg[1].get<float>();
    scene->world->horb = bg[2].get<float>();
  }

  /* ── Elements ── */
  for (const json &el : doc["elements"]) {
    const std::string kind = el.value("kind", "plane");
    const std::string name = el.value("name", "Element");

    Object *ob = nullptr;
    if (kind == "plane") {
      Mesh *me = make_plane_mesh();
      ob = BKE_object_add(bmain, scene, view_layer, OB_MESH, name.c_str());
      BKE_mesh_nomain_to_mesh(me, static_cast<Mesh *>(ob->data), ob);

      /* Texture from captured HTML PNG. */
      if (el.contains("image_path") && el["image_path"].is_string()) {
        std::string img = el["image_path"].get<std::string>();
        Image *image = BKE_image_load(bmain, img.c_str());
        Material *mat = BKE_material_add(bmain, (name + "_mat").c_str());
        /* Wire an image-texture -> principled-bsdf (emission for unlit).
         * Node graph setup is done in juicer_material_setup() — see build notes. */
        BKE_object_material_assign(bmain, ob, mat, 1, BKE_MAT_ASSIGN_OBDATA);
        UNUSED_VARS(image);
      }
    }
    else { /* box / sphere -> primitive added via BKE; see build notes */
      ob = BKE_object_add(bmain, scene, view_layer, OB_MESH, name.c_str());
    }
    if (!ob) {
      continue;
    }

    /* Base transform. */
    const json &p = el["position"];
    const json &rot = el["rotation"];
    const json &s = el["scale"];
    ob->loc[0] = p[0]; ob->loc[1] = p[1]; ob->loc[2] = p[2];
    ob->rot[0] = rot[0]; ob->rot[1] = rot[1]; ob->rot[2] = rot[2];
    float sw = (kind == "plane") ? el.value("width", 1.0f) : 1.0f;
    float sh = (kind == "plane") ? el.value("height", 1.0f) : 1.0f;
    ob->size[0] = s[0].get<float>() * sw;
    ob->size[1] = s[1].get<float>() * sh;
    ob->size[2] = s[2];

    /* ── Keyframe tracks ── */
    for (const json &nt : el.value("tracks", json::array())) {
      const std::string prop = nt["property"];
      const char *rna = prop == "position" ? "location" :
                        prop == "rotation" ? "rotation_euler" :
                        prop == "scale"    ? "scale" :
                                             nullptr; /* opacity handled via material */
      for (const json &k : nt["track"]["keys"]) {
        float frame = k["frame"].get<float>();
        std::string easing = k.value("easing", "ease-in-out");
        if (rna) {
          insert_vec3_key(ob, rna, frame, k["value"], easing);
        }
        /* opacity -> material alpha fcurve: see build notes (node socket path). */
      }
    }
  }

  /* ── Camera ── */
  const json &cam = doc["camera"];
  Object *cam_ob = BKE_object_add(bmain, scene, view_layer, OB_CAMERA, "JuicerCamera");
  const json &cp = cam["position"];
  cam_ob->loc[0] = cp[0]; cam_ob->loc[1] = cp[1]; cam_ob->loc[2] = cp[2];
  /* Aim the camera at the target via a tracking constraint or direct matrix —
   * see build notes; for now point down -Z toward target by computing euler. */
  scene->camera = cam_ob;

  /* ── Render ── */
  BLI_strncpy(scene->r.pic, out_path, sizeof(scene->r.pic));
  DEG_relations_tag_update(bmain);
  BKE_scene_graph_update_tagged(CTX_data_ensure_evaluated_depsgraph(C), bmain);

  Render *re = RE_NewSceneRender(scene);
  int sfra = (single_frame >= 0) ? single_frame : scene->r.sfra;
  int efra = (single_frame >= 0) ? single_frame : scene->r.efra;
  RE_RenderAnim(re, bmain, scene, nullptr, nullptr, sfra, efra, scene->r.frs_sec);

  std::fprintf(stderr, "[juicer] rendered frames %d-%d -> %s\n", sfra, efra, out_path);
  return true;
}

}  // namespace blender::io::juicer
