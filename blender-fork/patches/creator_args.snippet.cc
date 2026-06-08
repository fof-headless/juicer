/* ─────────────────────────────────────────────────────────────────────────────
 * JUICER FORK — CLI flag `--juicer <scene.json>`
 *
 * build-fork.sh injects:
 *   1. this handler function, before `setupArguments()` in creator_args.cc
 *   2. the BLI_args_add registration line (below) inside setupArguments()
 *   3. `#include "IO_juicer.hh"` near the top of creator_args.cc
 * ──────────────────────────────────────────────────────────────────────────── */

static int arg_handle_juicer_scene(int argc, const char **argv, void *data)
{
  bContext *C = static_cast<bContext *>(data);
  if (argc < 2) {
    fprintf(stderr, "\nError: --juicer requires <scene.json> [--juicer-out <path>] \n");
    return 0;
  }
  char json_path[FILE_MAX];
  STRNCPY(json_path, argv[1]);
  BLI_path_canonicalize_native(json_path, sizeof(json_path));

  /* Output path comes from the standard -o/--render-output, already parsed. */
  Scene *scene = CTX_data_scene(C);
  const char *out = scene ? scene->r.pic : "//juicer_out/";

  /* single-frame override via env JUICER_FRAME (set by Juicer for previews). */
  int single = -1;
  if (const char *f = getenv("JUICER_FRAME")) {
    single = atoi(f);
  }

  bool ok = blender::io::juicer::import_and_render(C, json_path, out, single);
  if (!ok && app_state.exit_code_on_error.python) {
    WM_exit(C, app_state.exit_code_on_error.python);
  }
  return 1; /* consume the scene.json argument */
}

/* Registration line — injected into setupArguments(), next to the --python add:
 *
 *   BLI_args_add(ba, nullptr, "--juicer", CB(arg_handle_juicer_scene), C);
 */
