# voice lane: included after every project() while transcribe-cpp-sys builds
# transcribe.cpp and its ggml. The workflows pass it as
# TRANSCRIBE_CMAKE_ARGS=-DCMAKE_PROJECT_INCLUDE=<this file>
# (tools/voice-cmake-env.mjs), and src-tauri/build.rs stops a build that
# lacks it.
#
# 1. The static C++ runtime (-MT). tauri-build links the VC runtime
#    statically, so the exe runs on a PC without the Visual C++
#    redistributable; transcribe.cpp built -MD would bring back a load-time
#    dependency on msvcp140.dll. cmake-rs writes cc's -MD into the flag
#    variables, so it is swapped here.
# 2. AVX2, FMA and F16C, not the build machine's CPU: GGML_NATIVE on a CI
#    runner can pick AVX-512, which most PCs would crash on. voice/whisper.rs
#    checks the CPU for these before it loads a model.
if(MSVC)
  foreach(flags
      CMAKE_C_FLAGS CMAKE_CXX_FLAGS
      CMAKE_C_FLAGS_RELEASE CMAKE_CXX_FLAGS_RELEASE
      CMAKE_C_FLAGS_RELWITHDEBINFO CMAKE_CXX_FLAGS_RELWITHDEBINFO
      CMAKE_C_FLAGS_MINSIZEREL CMAKE_CXX_FLAGS_MINSIZEREL
      CMAKE_C_FLAGS_DEBUG CMAKE_CXX_FLAGS_DEBUG)
    string(REGEX REPLACE "[-/]MDd?( |$)" "-MT\\1" ${flags} "${${flags}}")
  endforeach()
  set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded")
  # 3. Optimisation. cmake-rs hands CMake cc's flags for the build type
  #    (CMAKE_C_FLAGS_RELEASE and so on), and asks cc for them at opt-level
  #    0, so they carry no /O at all and MSVC compiled ggml unoptimised:
  #    Whisper ran about 30 times slower than it should
  #    (entries/voice-speed.md, D-voice-15). /Od goes, and /O2 goes in.
  #    transcribe-cpp-sys adds /O2 itself; this stays as the second lock.
  foreach(flags
      CMAKE_C_FLAGS CMAKE_CXX_FLAGS
      CMAKE_C_FLAGS_RELEASE CMAKE_CXX_FLAGS_RELEASE
      CMAKE_C_FLAGS_RELWITHDEBINFO CMAKE_CXX_FLAGS_RELWITHDEBINFO
      CMAKE_C_FLAGS_MINSIZEREL CMAKE_CXX_FLAGS_MINSIZEREL)
    string(REGEX REPLACE "[-/]Od( |$)" "\\1" ${flags} "${${flags}}")
  endforeach()
  foreach(flags
      CMAKE_C_FLAGS_RELEASE CMAKE_CXX_FLAGS_RELEASE
      CMAKE_C_FLAGS_RELWITHDEBINFO CMAKE_CXX_FLAGS_RELWITHDEBINFO
      CMAKE_C_FLAGS_MINSIZEREL CMAKE_CXX_FLAGS_MINSIZEREL)
    if(NOT "${${flags}}" MATCHES "[-/]O2")
      set(${flags} "${${flags}} -O2 -Ob2 -DNDEBUG")
    endif()
  endforeach()
endif()

set(GGML_NATIVE OFF CACHE BOOL "voice lane: a portable build" FORCE)
set(GGML_AVX ON CACHE BOOL "voice lane" FORCE)
set(GGML_AVX2 ON CACHE BOOL "voice lane" FORCE)
set(GGML_FMA ON CACHE BOOL "voice lane" FORCE)
set(GGML_F16C ON CACHE BOOL "voice lane" FORCE)
set(GGML_AVX512 OFF CACHE BOOL "voice lane" FORCE)
