# SPDX-FileCopyrightText: 2026 Juicer
#
# SPDX-License-Identifier: GPL-2.0-or-later

set(INC
  .
  importer
  ../common
  ../../blenkernel
  ../../animrig
  ../../makesdna
  ../../makesrna
  ../../render
  ../../depsgraph
)

set(INC_SYS
  ../../../../extern/json/include
)

set(SRC
  importer/juicer_scene.cc

  IO_juicer.hh
)

set(LIB
  bf_blenkernel
  PRIVATE bf::animrig
  PRIVATE bf::blenlib
  PRIVATE bf::depsgraph
  PRIVATE bf::dna
  PRIVATE bf::intern::guardedalloc
  bf_render
)

blender_add_lib(bf_io_juicer "${SRC}" "${INC}" "${INC_SYS}" "${LIB}")
