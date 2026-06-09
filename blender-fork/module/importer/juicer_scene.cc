/* SPDX-FileCopyrightText: 2026 Juicer
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

/** \file
 * \ingroup juicer
 *
 * Native importer: Juicer scene.json -> Blender scene -> render.
 *
 * Uses Blender's real subsystems — BKE objects/meshes/materials, animrig
 * F-curve API, RE_ render pipeline. No Python (bpy).
 *
 * Pinned against Blender commit 1957ef3 (v5.03 alpha).
 */

#include "IO_juicer.hh"

#include <json.hpp> /* extern/json/include/json.hpp (nlohmann) */

#include <cmath>
#include <cstdio>
#include <string>
#include <vector>

#include "BLI_math_matrix.h"
#include "BLI_math_rotation.h"
#include "BLI_string.h"

#include "DNA_constraint_types.h"
#include "DNA_curve_types.h"
#include "DNA_light_types.h"
#include "DNA_material_types.h"
#include "DNA_node_types.h"
#include "DNA_object_types.h"
#include "DNA_scene_types.h"
#include "DNA_world_types.h"

#include "BKE_constraint.h"
#include "BKE_context.hh"
#include "BKE_idprop.h"
#include "BKE_image.hh"
#include "BKE_lib_id.hh"
#include "BKE_light.hh"
#include "BKE_main.hh"
#include "BKE_material.hh"
#include "BKE_mesh.hh"
#include "BKE_node.hh"
#include "BKE_node_runtime.hh"
#include "BKE_object.hh"
#include "BKE_scene.hh"

#include "NOD_shader.h"

#include "ANIM_animdata.hh"
#include "ANIM_fcurve.hh"

#include "DEG_depsgraph.hh"
#include "DEG_depsgraph_build.hh"

#include "ED_node.hh"

#include "RE_pipeline.h"

using json = nlohmann::json;

namespace blender::io::juicer {

/* ── Helpers ──────────────────────────────────────────────────────────────── */

static float srgb_to_linear(float v)
{
  return std::pow(std::max(v, 0.0f), 2.2f);
}

static void hex_to_linear_rgb(const std::string &hex, float out[3])
{
  const char *h = hex.c_str();
  if (*h == '#') h++;
  if (strlen(h) < 6) {
    out[0] = out[1] = out[2] = 0.5f;
    return;
  }
  auto parse2 = [](const char *s) { return (float)std::strtol(std::string(s, 2).c_str(), nullptr, 16) / 255.0f; };
  out[0] = srgb_to_linear(parse2(h));
  out[1] = srgb_to_linear(parse2(h + 2));
  out[2] = srgb_to_linear(parse2(h + 4));
}

/* ── Easing map ───────────────────────────────────────────────────────────── */

struct IpoMap {
  char ipo;    /* eBezTriple_Interpolation */
  char easing; /* eBezTriple_Easing */
};

static IpoMap map_easing(const std::string &e)
{
  if (e == "linear") return {BEZT_IPO_LIN, BEZT_IPO_EASE_AUTO};
  if (e == "step") return {BEZT_IPO_CONST, BEZT_IPO_EASE_AUTO};
  if (e == "ease-in") return {BEZT_IPO_CUBIC, BEZT_IPO_EASE_IN};
  if (e == "ease-out") return {BEZT_IPO_CUBIC, BEZT_IPO_EASE_OUT};
  return {BEZT_IPO_CUBIC, BEZT_IPO_EASE_IN_OUT};
}

/* ── Mesh helpers ─────────────────────────────────────────────────────────── */

/** Unit quad in XY plane — scaled by ob->size for width/height. */
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

/** Unit cube (6 quads). Scaled via ob->size. */
static Mesh *make_box_mesh()
{
  Mesh *me = BKE_mesh_new_nomain(8, 0, 6, 24);
  blender::MutableSpan<blender::float3> p = me->vert_positions_for_write();
  p[0] = {-0.5f, -0.5f, -0.5f}; p[1] = {0.5f, -0.5f, -0.5f};
  p[2] = {0.5f,  0.5f, -0.5f};  p[3] = {-0.5f, 0.5f, -0.5f};
  p[4] = {-0.5f, -0.5f,  0.5f}; p[5] = {0.5f, -0.5f,  0.5f};
  p[6] = {0.5f,  0.5f,  0.5f};  p[7] = {-0.5f, 0.5f,  0.5f};

  static const int quads[6][4] = {
    {0,3,2,1}, {4,5,6,7}, {0,1,5,4}, {2,3,7,6}, {0,4,7,3}, {1,2,6,5}
  };
  blender::MutableSpan<int> fo = me->face_offsets_for_write();
  blender::MutableSpan<int> cv = me->corner_verts_for_write();
  for (int fi = 0; fi < 6; fi++) {
    fo[fi] = fi * 4;
    for (int li = 0; li < 4; li++) cv[fi * 4 + li] = quads[fi][li];
  }
  fo[6] = 24;

  BKE_mesh_calc_edges(me, false, false);
  BKE_mesh_calc_normals(me);
  return me;
}

/** UV sphere, 16 segments × 8 rings. */
static Mesh *make_sphere_mesh()
{
  const int segs = 16, rings = 8;
  const int verts_num = 2 + segs * rings;
  const int top_tris = segs, bot_tris = segs;
  const int body_quads = segs * (rings - 1);
  const int faces_num = top_tris + bot_tris + body_quads;
  const int loops_num = top_tris * 3 + bot_tris * 3 + body_quads * 4;

  Mesh *me = BKE_mesh_new_nomain(verts_num, 0, faces_num, loops_num);
  blender::MutableSpan<blender::float3> pos = me->vert_positions_for_write();

  pos[0] = {0, 0, 1.0f}; /* top pole */
  int vi = 1;
  for (int r = 0; r < rings; r++) {
    float phi = M_PI * (r + 1) / (rings + 1);
    float z = std::cos(phi), rr = std::sin(phi);
    for (int s = 0; s < segs; s++) {
      float theta = 2.0f * M_PI * s / segs;
      pos[vi++] = {rr * std::cos(theta), rr * std::sin(theta), z};
    }
  }
  pos[vi] = {0, 0, -1.0f}; /* bottom pole */
  int bot_pole = vi;

  auto ring_v = [&](int r, int s) { return 1 + r * segs + (s % segs); };

  blender::MutableSpan<int> fo = me->face_offsets_for_write();
  blender::MutableSpan<int> cv = me->corner_verts_for_write();
  int fi = 0, li = 0;

  /* Top cap */
  for (int s = 0; s < segs; s++, fi++) {
    fo[fi] = li;
    cv[li++] = 0; cv[li++] = ring_v(0, s + 1); cv[li++] = ring_v(0, s);
  }
  /* Body */
  for (int r = 0; r < rings - 1; r++) {
    for (int s = 0; s < segs; s++, fi++) {
      fo[fi] = li;
      cv[li++] = ring_v(r, s); cv[li++] = ring_v(r, s + 1);
      cv[li++] = ring_v(r + 1, s + 1); cv[li++] = ring_v(r + 1, s);
    }
  }
  /* Bottom cap */
  for (int s = 0; s < segs; s++, fi++) {
    fo[fi] = li;
    cv[li++] = bot_pole; cv[li++] = ring_v(rings - 1, s); cv[li++] = ring_v(rings - 1, s + 1);
  }
  fo[fi] = li;

  BKE_mesh_calc_edges(me, false, false);
  BKE_mesh_calc_normals(me);
  return me;
}

/* ── Material helpers ─────────────────────────────────────────────────────── */

/**
 * Unlit emission material:
 *   [Image Texture] → Emission.Color → [Mix Shader (factor=opacity)] → Surface
 *                                       Transparent BSDF ──────────────┘
 * The MixShader factor is driven by object custom property "opacity"
 * so opacity F-curves can target "[\"opacity\"]" on the Object.
 */
static Material *make_unlit_material(Main *bmain,
                                     const std::string &name,
                                     const float color[3],
                                     float opacity,
                                     const std::string &image_path)
{
  Material *mat = BKE_material_add(bmain, name.c_str());
  mat->use_nodes = true;
  mat->blend_method = MA_BM_BLEND;

  bNodeTree *ntree = ntreeAddTreeEmbedded(nullptr, &mat->id, "Shader Nodetree", "ShaderNodeTree");
  mat->nodetree = ntree;
  ntreeSetTypes(nullptr, ntree);

  /* Output */
  bNode *out = nodeAddStaticNode(nullptr, ntree, SH_NODE_OUTPUT_MATERIAL);
  out->locx = 420; out->locy = 0;

  /* Transparent BSDF */
  bNode *transp = nodeAddStaticNode(nullptr, ntree, SH_NODE_BSDF_TRANSPARENT);
  transp->locx = 0; transp->locy = -140;

  /* Mix Shader (factor = opacity) */
  bNode *mix = nodeAddStaticNode(nullptr, ntree, SH_NODE_MIX_SHADER);
  mix->locx = 220; mix->locy = 0;
  {
    bNodeSocket *fac = (bNodeSocket *)BLI_findlink(&mix->inputs, 0);
    if (fac) ((bNodeSocketValueFloat *)fac->default_value)->value = opacity;
  }

  /* Emission */
  bNode *emit = nodeAddStaticNode(nullptr, ntree, SH_NODE_EMISSION);
  emit->locx = 0; emit->locy = 60;

  /* Strength = 1 */
  bNodeSocket *strength = nodeFindSocket(emit, SOCK_IN, "Strength");
  if (strength) ((bNodeSocketValueFloat *)strength->default_value)->value = 1.0f;

  bNodeSocket *emit_col = nodeFindSocket(emit, SOCK_IN, "Color");

  if (!image_path.empty()) {
    /* Image Texture → Emission color */
    bNode *tex = nodeAddStaticNode(nullptr, ntree, SH_NODE_TEX_IMAGE);
    tex->locx = -200; tex->locy = 60;
    Image *img = BKE_image_load_exists(bmain, image_path.c_str());
    if (img) tex->id = &img->id;

    bNodeSocket *tex_col = nodeFindSocket(tex, SOCK_OUT, "Color");
    bNodeSocket *tex_alpha = nodeFindSocket(tex, SOCK_OUT, "Alpha");
    bNodeSocket *fac_sock = (bNodeSocket *)BLI_findlink(&mix->inputs, 0);

    if (tex_col && emit_col) nodeAddLink(ntree, tex, tex_col, emit, emit_col);
    /* Use texture alpha to modulate the mix factor as well */
    if (tex_alpha && fac_sock) nodeAddLink(ntree, tex, tex_alpha, mix, fac_sock);
  }
  else if (emit_col) {
    auto *rgba = (bNodeSocketValueRGBA *)emit_col->default_value;
    rgba->value[0] = color[0]; rgba->value[1] = color[1];
    rgba->value[2] = color[2]; rgba->value[3] = 1.0f;
  }

  /* Wire: transp → mix[1], emit → mix[2], mix → output */
  bNodeSocket *transp_out = nodeFindSocket(transp, SOCK_OUT, "BSDF");
  bNodeSocket *emit_out   = nodeFindSocket(emit,   SOCK_OUT, "Emission");
  bNodeSocket *mix_in1    = (bNodeSocket *)BLI_findlink(&mix->inputs, 1);
  bNodeSocket *mix_in2    = (bNodeSocket *)BLI_findlink(&mix->inputs, 2);
  bNodeSocket *mix_out    = nodeFindSocket(mix, SOCK_OUT, "Shader");
  bNodeSocket *out_surf   = nodeFindSocket(out, SOCK_IN, "Surface");

  if (transp_out && mix_in1) nodeAddLink(ntree, transp, transp_out, mix, mix_in1);
  if (emit_out   && mix_in2) nodeAddLink(ntree, emit, emit_out, mix, mix_in2);
  if (mix_out    && out_surf) nodeAddLink(ntree, mix, mix_out, out, out_surf);

  ntreeUpdateAllNew(bmain);
  return mat;
}

/**
 * Add a custom float property "opacity" to the object and install a driver
 * on the MixShader factor socket that reads it.
 * This lets us insert F-curve keys on ob["opacity"] to animate fade.
 */
static void setup_opacity_driver(Main *bmain, Object *ob, Material *mat)
{
  /* Custom property */
  IDPropertyTemplate fval = {};
  fval.f = 1.0f;
  IDProperty *prop = IDP_New(IDP_FLOAT, &fval, "opacity");
  if (!ob->id.properties) {
    IDPropertyTemplate gval = {};
    ob->id.properties = IDP_New(IDP_GROUP, &gval, "id_properties");
  }
  IDP_AddToGroup(ob->id.properties, prop);

  if (!mat || !mat->nodetree) return;

  /* Find MixShader node */
  bNode *mix = nullptr;
  LISTBASE_FOREACH (bNode *, n, &mat->nodetree->nodes) {
    if (n->type == SH_NODE_MIX_SHADER) { mix = n; break; }
  }
  if (!mix) return;

  bNodeSocket *fac = (bNodeSocket *)BLI_findlink(&mix->inputs, 0);
  if (!fac) return;

  /* Build driver on material's node socket via AnimData */
  char node_path[512];
  BLI_snprintf(node_path, sizeof(node_path),
               "node_tree.nodes[\"%s\"].inputs[0].default_value", mix->name);

  FCurve *fcu = BKE_fcurve_create();
  fcu->rna_path = BLI_strdup(node_path);
  fcu->array_index = 0;

  ChannelDriver *drv = (ChannelDriver *)MEM_callocN(sizeof(ChannelDriver), "juicer_opacity_drv");
  drv->type = DRIVER_TYPE_AVERAGE;
  fcu->driver = drv;

  DriverVar *dvar = driver_add_new_variable(drv);
  STRNCPY(dvar->name, "opacity");
  dvar->type = DVAR_TYPE_SINGLE_PROP;
  DriverTarget *tgt = &dvar->targets[0];
  tgt->id = &ob->id;
  tgt->id_type = ID_OB;
  tgt->rna_path = BLI_strdup("[\"opacity\"]");

  AnimData *adt = BKE_animdata_ensure_id(&mat->id);
  if (adt) {
    BLI_addtail(&adt->drivers, fcu);
    DEG_id_tag_update(&mat->id, ID_RECALC_ANIMATION);
  }

  UNUSED_VARS(bmain);
}

/* ── Keyframe insertion ───────────────────────────────────────────────────── */

static void insert_key(Object *ob,
                       const char *rna_path,
                       int array_index,
                       float frame,
                       float value,
                       const std::string &easing)
{
  using namespace blender::animrig;
  FCurve *fcu = action_fcurve_ensure_ex(nullptr,
                                         nullptr,
                                         nullptr,
                                         nullptr,
                                         FCurveDescriptor{rna_path, array_index});
  if (!fcu) return;

  KeyframeSettings settings = get_keyframe_settings(false);
  IpoMap m = map_easing(easing);
  settings.interpolation = static_cast<eBezTriple_Interpolation>(m.ipo);
  insert_vert_fcurve(fcu, {frame, value}, settings, INSERTKEY_NOFLAGS);

  if (fcu->totvert > 0) {
    BezTriple &bz = fcu->bezt[fcu->totvert - 1];
    bz.ipo = m.ipo;
    bz.easing = m.easing;
  }

  /* Attach fcu to the object's AnimData if it isn't already there. */
  AnimData *adt = BKE_animdata_ensure_id(&ob->id);
  if (adt && adt->action) {
    /* action_fcurve_ensure_ex may have already added it; noop if so. */
  }
  UNUSED_VARS(ob);
}

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

/* ── Main importer ────────────────────────────────────────────────────────── */

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
  if (doc.contains("render")) {
    const json &r = doc["render"];
    scene->r.xsch = r.value("width", 1920);
    scene->r.ysch = r.value("height", 1080);
    scene->r.frs_sec = r.value("fps", 30);
    scene->r.frs_sec_base = 1.0f;
    scene->r.sfra = r.value("frame_start", 1);
    scene->r.efra = r.value("frame_end", 250);
    scene->r.size = 100;

    if (r.contains("background") && scene->world) {
      const json &bg = r["background"];
      scene->world->horr = bg[0].get<float>();
      scene->world->horg = bg[1].get<float>();
      scene->world->horb = bg[2].get<float>();
    }
  }

  /* ── Directional light ── */
  if (doc.contains("light")) {
    const json &jl = doc["light"];
    Object *light_ob = BKE_object_add(bmain, scene, view_layer, OB_LAMP, "JuicerSun");
    Light *ld = (Light *)light_ob->data;
    ld->type = LA_SUN;
    ld->energy = jl.value("intensity", 1.2f);
    if (jl.contains("color")) {
      ld->r = jl["color"][0].get<float>();
      ld->g = jl["color"][1].get<float>();
      ld->b = jl["color"][2].get<float>();
    }
    if (jl.contains("direction")) {
      float d[3] = {jl["direction"][0], jl["direction"][1], jl["direction"][2]};
      float quat[4];
      float ref[3] = {0.0f, 0.0f, -1.0f};
      normalize_v3(d);
      negate_v3(d); /* point -Z toward light direction */
      rotation_between_vecs_to_quat(quat, ref, d);
      quat_to_eulO(light_ob->rot, EULER_ORDER_DEFAULT, quat);
    }
  }

  /* ── Elements ── */
  for (const json &el : doc.value("elements", json::array())) {
    const std::string kind = el.value("kind", "plane");
    const std::string name = el.value("name", "Element");
    bool unlit = el.value("unlit", kind == "plane");
    float opacity = el.value("opacity", 1.0f);
    std::string color_hex = el.value("color", "#4488ff");
    std::string img_path;
    if (el.contains("image_path") && el["image_path"].is_string()) {
      img_path = el["image_path"].get<std::string>();
    }

    float color[3];
    hex_to_linear_rgb(color_hex, color);

    Object *ob = BKE_object_add(bmain, scene, view_layer, OB_MESH, name.c_str());

    if (kind == "plane") {
      Mesh *me = make_plane_mesh();
      BKE_mesh_nomain_to_mesh(me, static_cast<Mesh *>(ob->data), ob);
    }
    else if (kind == "box") {
      Mesh *me = make_box_mesh();
      BKE_mesh_nomain_to_mesh(me, static_cast<Mesh *>(ob->data), ob);
    }
    else { /* sphere */
      Mesh *me = make_sphere_mesh();
      BKE_mesh_nomain_to_mesh(me, static_cast<Mesh *>(ob->data), ob);
    }

    /* Visibility */
    if (!el.value("visible", true)) {
      ob->visibility_flag |= OB_HIDE_VIEWPORT | OB_HIDE_RENDER;
    }

    /* Base transform */
    if (el.contains("position")) {
      const json &p = el["position"];
      ob->loc[0] = p[0]; ob->loc[1] = p[1]; ob->loc[2] = p[2];
    }
    if (el.contains("rotation")) {
      const json &rot = el["rotation"];
      ob->rot[0] = rot[0]; ob->rot[1] = rot[1]; ob->rot[2] = rot[2];
    }
    if (el.contains("scale")) {
      const json &s = el["scale"];
      float sw = (kind == "plane") ? el.value("width", 1.0f) : 1.0f;
      float sh = (kind == "plane") ? el.value("height", 1.0f) : 1.0f;
      ob->size[0] = s[0].get<float>() * sw;
      ob->size[1] = s[1].get<float>() * sh;
      ob->size[2] = s[2].get<float>();
    }

    /* Material */
    Material *mat = nullptr;
    if (unlit) {
      mat = make_unlit_material(bmain, name + "_mat", color, opacity, img_path);
      setup_opacity_driver(bmain, ob, mat);
    }
    else {
      mat = BKE_material_add(bmain, (name + "_mat").c_str());
      mat->use_nodes = true;
      ED_node_shader_default(nullptr, &mat->id);
      /* Tint the Principled BSDF base color */
      if (mat->nodetree) {
        LISTBASE_FOREACH (bNode *, n, &mat->nodetree->nodes) {
          if (n->type == SH_NODE_BSDF_PRINCIPLED) {
            bNodeSocket *base = nodeFindSocket(n, SOCK_IN, "Base Color");
            if (base) {
              auto *rgba = (bNodeSocketValueRGBA *)base->default_value;
              rgba->value[0] = color[0]; rgba->value[1] = color[1];
              rgba->value[2] = color[2]; rgba->value[3] = opacity;
            }
            break;
          }
        }
      }
    }
    if (mat) BKE_object_material_assign(bmain, ob, mat, 1, BKE_MAT_ASSIGN_OBDATA);

    /* ── Keyframe tracks ── */
    for (const json &nt : el.value("tracks", json::array())) {
      const std::string prop = nt.value("property", "");
      for (const json &k : nt["track"]["keys"]) {
        float frame = k["frame"].get<float>();
        std::string easing = k.value("easing", "ease-in-out");

        if (prop == "position") {
          insert_vec3_key(ob, "location", frame, k["value"], easing);
        }
        else if (prop == "rotation") {
          insert_vec3_key(ob, "rotation_euler", frame, k["value"], easing);
        }
        else if (prop == "scale") {
          insert_vec3_key(ob, "scale", frame, k["value"], easing);
        }
        else if (prop == "opacity") {
          /* Key the custom property; driver routes it to the MixShader factor. */
          float val = k["value"].is_array() ? k["value"][0].get<float>() : k["value"].get<float>();
          insert_key(ob, "[\"opacity\"]", 0, frame, val, easing);
        }
      }
    }
  }

  /* ── Camera ── */
  if (doc.contains("camera")) {
    const json &cam = doc["camera"];
    Object *cam_ob = BKE_object_add(bmain, scene, view_layer, OB_CAMERA, "JuicerCamera");
    scene->camera = cam_ob;

    /* FoV → focal length */
    Camera *cam_data = (Camera *)cam_ob->data;
    float fov_deg = cam.value("fov_deg", 45.0f);
    float fov_rad = fov_deg * (float)M_PI / 180.0f;
    cam_data->lens = cam_data->sensor_x / (2.0f * std::tan(fov_rad * 0.5f));
    cam_data->clip_start = cam.value("near", 0.1f);
    cam_data->clip_end   = cam.value("far", 100.0f);

    if (cam.contains("position")) {
      const json &p = cam["position"];
      cam_ob->loc[0] = p[0]; cam_ob->loc[1] = p[1]; cam_ob->loc[2] = p[2];
    }

    /* Track-To constraint: camera points at target Empty */
    if (cam.contains("target")) {
      const json &t = cam["target"];
      Object *target = BKE_object_add(bmain, scene, view_layer, OB_EMPTY, "JuicerCamTarget");
      target->loc[0] = t[0]; target->loc[1] = t[1]; target->loc[2] = t[2];

      bConstraint *con = BKE_constraint_add_for_object(cam_ob, "TrackTo", CONSTRAINT_TYPE_TRACKTO);
      bTrackToConstraint *tt = (bTrackToConstraint *)con->data;
      tt->tar = target;
      tt->reserved1 = TRACK_nZ; /* camera looks along -Z */
      tt->reserved2 = UP_Y;
    }
  }

  /* ── Output path & render ── */
  BLI_strncpy(scene->r.pic, out_path, sizeof(scene->r.pic));

  DEG_relations_tag_update(bmain);
  BKE_scene_graph_update_tagged(CTX_data_ensure_evaluated_depsgraph(C), bmain);

  Render *re = RE_NewSceneRender(scene);
  int sfra = (single_frame >= 0) ? single_frame : scene->r.sfra;
  int efra = (single_frame >= 0) ? single_frame : scene->r.efra;
  RE_RenderAnim(re, bmain, scene, nullptr, nullptr, sfra, efra, scene->r.frs_sec);

  std::fprintf(stderr, "[juicer] rendered frames %d–%d → %s\n", sfra, efra, out_path);
  return true;
}

} /* namespace blender::io::juicer */
