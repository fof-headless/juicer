/* ─────────────────────────────────────────────────────────────────────────────
 * JUICER FORK — CLI flag `--juicer <scene.json>`
 *
 * inject_cli.py splices this entire block just before main_args_setup() in
 * creator_args.cc.  The forward declaration makes the linker find the symbol
 * without needing IO_juicer.hh in the creator's -I paths.
 * ──────────────────────────────────────────────────────────────────────────── */

/* Forward declaration — defined in bf_io_juicer (source/blender/io/juicer). */
struct bContext;
namespace blender::io::juicer {
bool import_and_render(bContext *C,
                       const char *json_path,
                       const char *out_path,
                       int single_frame);
}

static const char arg_handle_juicer_scene_doc[] =
    "<scene.json>\n"
    "\tImport a Juicer scene.json and render it natively (no Python).";
static int arg_handle_juicer_scene(int argc, const char **argv, void *data)
{
  bContext *C = static_cast<bContext *>(data);
  if (argc < 2) {
    fprintf(stderr, "\nError: --juicer requires <scene.json>\n");
    return 0;
  }
  char json_path[FILE_MAX];
  STRNCPY(json_path, argv[1]);
  BLI_path_canonicalize_native(json_path, sizeof(json_path));

  /* Output path: use the -o/--render-output value that was already parsed. */
  Scene *scene = CTX_data_scene(C);
  const char *out = (scene && scene->r.pic[0]) ? scene->r.pic : "/tmp/juicer_out/";

  int single = -1;
  if (const char *f = getenv("JUICER_FRAME")) {
    single = atoi(f);
  }

  bool ok = blender::io::juicer::import_and_render(C, json_path, out, single);
  exit(ok ? 0 : 1);
  return 1;
}
