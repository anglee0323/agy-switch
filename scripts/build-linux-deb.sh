#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

case "${1:---native}" in
  --docker)
    command -v docker >/dev/null || { echo 'Docker is required for --docker.' >&2; exit 1; }
    task_cache="${XDG_CACHE_HOME:-$HOME/.cache}/agy-switch-build"
    mkdir -p "$task_cache/cargo"
    docker build -f scripts/linux/Dockerfile -t agy-switch-linux-builder .
    docker run --rm --cpus "${CARGO_BUILD_JOBS:-4}" --user "$(id -u):$(id -g)" \
      -e HOME=/tmp/builder -e CARGO_HOME=/cargo -e CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}" \
      -v "$PWD:/work" -v "$task_cache/cargo:/cargo" -w /work \
      agy-switch-linux-builder ./scripts/build-linux-deb.sh --native
    ;;
  --native)
    for task_tool in cargo node npm pkg-config dpkg-deb; do
      command -v "$task_tool" >/dev/null || { echo "Missing $task_tool. See docs/linux.md or use --docker." >&2; exit 1; }
    done
    pkg-config --exists gtk+-3.0 webkit2gtk-4.1 ayatana-appindicator3-0.1 || {
      echo 'Missing Linux development libraries. See docs/linux.md or use --docker.' >&2; exit 1;
    }
    npm ci
    npm run tauri build -- --bundles deb -- --locked
    ;;
  *) echo 'Usage: scripts/build-linux-deb.sh [--native|--docker]' >&2; exit 2 ;;
esac
