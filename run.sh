#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
export CAMY_ROOT="$PWD"
selected_helper=${CAMY_HELPER_BINARY:-}

die() { printf '%s\n' "$@" >&2; exit 1; }

if [[ -n $selected_helper && ! -x $selected_helper ]]; then
  die "Selected helper is not executable: $selected_helper"
fi

# Plugin bootstrap: no build and no second watch process. A checkout
# with a built helper stays offline. Otherwise the pinned release installer
# fetches once, verifies the committed sha256, and installs under
# $XDG_DATA_HOME/camy/bin (docs/RELEASING.md).
print_path=0
ensure=0
case ${1:-} in
  --ensure) ensure=1 ;;
  --print-path) print_path=1; ensure=1 ;;
  "") ;;
  *) die "Usage: bash run.sh [--ensure|--print-path]" ;;
esac

install_desktop_and_window() {
  bash scripts/write-desktop-entry.sh >/dev/null || true
  bash scripts/install-window-rules.sh >/dev/null || true
}

if [[ $ensure == 1 ]]; then
  if [[ -n ${CAMY_BOOTSTRAP_LOG:-} ]]; then
    mkdir -p "$(dirname "$CAMY_BOOTSTRAP_LOG")"
    exec 2> "$CAMY_BOOTSTRAP_LOG"
  fi
  trap install_desktop_and_window EXIT
  if [[ -n $selected_helper ]]; then
    [[ $print_path == 1 ]] && printf '%s\n' "$selected_helper"
    exit 0
  fi
  if [[ -x bin/camy && ! -L bin/camy ]]; then
    [[ $print_path == 1 ]] && printf '%s\n' "$PWD/bin/camy"
    exit 0
  fi
  debug=${CARGO_TARGET_DIR:-$PWD/helper/target}/debug/camy
  if [[ -x $debug && ! -L $debug ]]; then
    [[ $print_path == 1 ]] && printf '%s\n' "$debug"
    exit 0
  fi
  helper=$(bash scripts/fetch-helper.sh --print-path)
  [[ $print_path == 1 ]] && printf '%s\n' "$helper"
  exit 0
fi

die "Usage: bash run.sh [--ensure|--print-path]"
