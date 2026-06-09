#!/usr/bin/env python3
"""Splice the Juicer --juicer CLI flag into Blender's creator_args.cc.

Idempotent: safe to run repeatedly. Inserts two things:
  1. The arg_handle_juicer_scene() handler + forward declaration (from snippet)
  2. BLI_args_add(... "--juicer" ...) inside main_args_setup()

(No separate #include injection — the forward declaration is embedded in the
snippet itself, avoiding include-path issues in the creator compile unit.)
"""
import re
import sys

def main(target: str, snippet_path: str) -> int:
    src = open(target, encoding="utf-8").read()
    snippet = open(snippet_path, encoding="utf-8").read()

    if "arg_handle_juicer_scene" in src:
        print("inject_cli: already patched")
        return 0

    # 1) handler + forward declaration — insert just before main_args_setup.
    m = re.search(r'\nvoid\s+main_args_setup\s*\(', src)
    if not m:
        m = re.search(r'\n(\w[\w\s\*:]*setupArguments\s*\()', src)
    if not m:
        print("inject_cli: could not find main_args_setup()/setupArguments()",
              file=sys.stderr)
        return 1
    src = src[:m.start()] + "\n" + snippet + "\n" + src[m.start():]

    # 2) registration — next to the --python registration line.
    reg = '  BLI_args_add(ba, nullptr, "--juicer", CB(arg_handle_juicer_scene), C);\n'
    src, n = re.subn(r'(BLI_args_add\(ba, "-P", "--python",[^\n]*\n)',
                     r'\1' + reg, src, count=1)
    if n == 0:
        print("inject_cli: could not find --python registration", file=sys.stderr)
        return 1

    open(target, "w", encoding="utf-8").write(src)
    print("inject_cli: patched", target)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))
