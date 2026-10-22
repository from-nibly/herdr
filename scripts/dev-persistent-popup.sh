#!/usr/bin/env bash
set -euo pipefail

# Run inside `nix develop`, or with the repository build tools on PATH.
cd "$(dirname "${BASH_SOURCE[0]}")/.."
cargo build --locked
bin="$(realpath "${CARGO_TARGET_DIR:-target}/debug/herdr")"
sandbox=$(mktemp -d /var/tmp/herdr-popup.XXXXXX)
session="popup-$$"
mkdir -p "$sandbox/config" "$sandbox/state"
cat > "$sandbox/config.toml" <<'TOML'
onboarding = false

[terminal]
default_shell = "/bin/sh"

[update]
version_check = false
manifest_check = false

[experimental]
allow_nested = true

[[keys.command]]
key = "f12"
type = "persistent_popup"
command = "exec /bin/sh"
scope = "workspace"
style = "drawer"
edge = "bottom"
height = "40%"

[[keys.command]]
key = "f11"
type = "persistent_popup"
command = "exec /bin/sh"
scope = "global"
style = "drawer"
edge = "top"
height = "40%"

[[keys.command]]
key = "f10"
type = "persistent_popup"
command = "exec /bin/sh"
scope = "tab"
style = "overlay"
width = "80%"
height = "60%"
TOML

run() {
  env -u HERDR_SOCKET_PATH -u HERDR_CLIENT_SOCKET_PATH -u HERDR_SESSION \
    -u HERDR_WORKSPACE_ID -u HERDR_TAB_ID -u HERDR_PANE_ID \
    XDG_CONFIG_HOME="$sandbox/config" XDG_STATE_HOME="$sandbox/state" \
    HERDR_CONFIG_PATH="$sandbox/config.toml" \
    "$bin" --session "$session" "$@"
}
cleanup() {
  run session stop "$session" || true
  printf '\nTest session stopped. Configuration and evidence retained at %s\n' "$sandbox"
}
run config check
printf 'Isolated test: %s\nF12 workspace/bottom; F11 global/top; F10 tab/overlay.\n' "$sandbox"
printf 'Only this disposable session is stopped when its client exits.\n'
trap cleanup EXIT
run
