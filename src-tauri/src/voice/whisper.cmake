# voice lane: included after every project() while whisper-rs-sys builds
# whisper.cpp (src-tauri/.cargo/config.toml sets CMAKE_PROJECT_INCLUDE).
#
# 1. The static C++ runtime (-MT). tauri-build links the VC runtime
#    statically, so the exe runs on a PC without the Visual C++
#    redistributable; whisper.cpp built -MD would bring back a load-time
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
endif()

set(GGML_NATIVE OFF CACHE BOOL "voice lane: a portable build" FORCE)
set(GGML_AVX ON CACHE BOOL "voice lane" FORCE)
set(GGML_AVX2 ON CACHE BOOL "voice lane" FORCE)
set(GGML_FMA ON CACHE BOOL "voice lane" FORCE)
set(GGML_F16C ON CACHE BOOL "voice lane" FORCE)
set(GGML_AVX512 OFF CACHE BOOL "voice lane" FORCE)
