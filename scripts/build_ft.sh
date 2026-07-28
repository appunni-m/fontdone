#!/bin/bash
# Build FreeType 2.14.3 in the ignored repository-local oracle directory.
# The build is consumed by fixture/oracle tools and is not installed.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
"${ROOT}/scripts/fetch_ft.sh"
cd "${ROOT}/freetype"
mkdir -p build && cd build
cmake_args=(
    -S ..
    -B .
    -DCMAKE_BUILD_TYPE=Release
    -DBUILD_SHARED_LIBS=ON
    -DFT_DISABLE_ZLIB=ON
    -DFT_DISABLE_PNG=ON
    -DFT_DISABLE_BZIP2=ON
    -DFT_DISABLE_BROTLI=ON
    -DFT_DISABLE_HARFBUZZ=ON
    "-DCMAKE_PROJECT_INCLUDE=${ROOT}/scripts/oracle_validator_modules.cmake"
)
if [ -f CMakeCache.txt ] &&
  ! grep -Fq "CMAKE_HOME_DIRECTORY:INTERNAL=${ROOT}/freetype" CMakeCache.txt; then
  # CMake caches absolute source/build paths. Reconfigure from scratch when a
  # copied checkout still points at its old location.
  cmake --fresh "${cmake_args[@]}"
else
  # Reconfigure every time so changes to the maintained oracle module overlay
  # cannot be hidden by an older CMake cache.
  cmake "${cmake_args[@]}"
fi
if [ -n "${FONTDONE_BUILD_JOBS:-}" ]; then
  build_jobs="${FONTDONE_BUILD_JOBS}"
elif command -v nproc >/dev/null 2>&1; then
  build_jobs="$(nproc)"
elif command -v sysctl >/dev/null 2>&1 &&
  build_jobs="$(sysctl -n hw.ncpu 2>/dev/null)"; then
  :
else
  build_jobs="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 1)"
fi
cmake --build . -j"${build_jobs}"
echo "FreeType 2.14.3 built at ${ROOT}/freetype/build"
