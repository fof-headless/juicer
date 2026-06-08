#!/usr/bin/env python3
"""Splice the Juicer --juicer CLI flag into Blender's creator_args.cc.

Idempotent: safe to run repeatedly. Inserts three things:
  1. #include "IO_juicer.hh"
  2. the arg_handle_juicer_scene() handler (from the snippet file)
  3. BLI_args_add(... "--juicer" ...) inside setupArguments()
"""
import re
import sys

def main(target: str, snippet_path: str) -> int:
    src = open(target, encoding="utf-8").read()
    snippet = open(snippet_path, encoding="utf-8").read()

    if "arg_handle_juicer_scene" in src:
        print("inject_cli: already patched")
        return 0

    # 1) include — after the first existing #include line.
    if '#include "IO_juicer.hh"' not in src:
        src = re.sub(r'(#include [^\n]+\n)',
                     r'\1#include "IO_juicer.hh"\n', src, count=1)

    # 2) handler — insert just before setupArguments definition.
    m = re.search(r'\n(\w[\w\s\*:]*setupArguments\s*\()', src)
    if not m:
        print("inject_cli: could not find setupArguments()", file=sys.stderr)
        return 1
    # Extract just the handler function from the snippet (between the marker comments).
    handler = snippet
    src = src[:m.start()] + "\n" + handler + "\n" + src[m.start():]

    # 3) registration — next to the --python registration line.
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
