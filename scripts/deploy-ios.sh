#!/usr/bin/env bash
#
# Builds Almena for one iOS destination and installs it there.
#
# The order matters and is the whole reason this is a script rather than three lines in the
# Taskfile: what gets built depends on where it is going. A phone needs an `aarch64` build and
# a simulator needs a simulator one, so the destination has to be chosen before the build
# starts, not after it.
#
# It also refuses to install an application that would not work once installed. `tauri ios
# build --debug` produces a bundle with no frontend in it, whose webview loads the interface
# over the network from the dev server on the developer's own computer. It runs perfectly on
# the desk it was built at and shows nothing anywhere else, which is exactly the kind of
# failure nobody catches. This script builds without `--debug` and then checks the artefact
# before handing it to a device.
#
# Run from the project root, which is where Task runs it. Set DEVICE to skip the prompt.

set -euo pipefail

# The bundle identifier, which has to agree with `identifier` in
# `src-tauri/tauri.conf.json`. Still the scaffold's guess — see spec 0001.
readonly BUNDLE_ID="network.almena.desktop"
readonly BUILD_DIR="src-tauri/gen/apple/build"

workspace="$(mktemp -d)"
trap 'rm -rf "$workspace"' EXIT

# ---------------------------------------------------------------------------
# What this Mac can install onto
# ---------------------------------------------------------------------------

# Each line is: kind, identifier, name, detail — separated by tabs.

paired_devices() {
  local json="$workspace/devices.json"
  xcrun devicectl list devices --json-output "$json" >/dev/null 2>&1 || return 0

  python3 - "$json" <<'PY'
import json
import sys

try:
    devices = json.load(open(sys.argv[1]))["result"]["devices"]
except Exception:
    devices = []

for device in devices:
    # An unpaired device cannot be installed onto, so it is not a destination. It is left out
    # rather than listed and refused later.
    if device.get("connectionProperties", {}).get("pairingState") != "paired":
        continue
    name = device.get("deviceProperties", {}).get("name", "unnamed")
    model = device.get("hardwareProperties", {}).get("marketingName", "")
    print("device\t%s\t%s\t%s" % (device["identifier"], name, model))
PY
}

available_simulators() {
  local json="$workspace/simulators.json"
  # Written to a file rather than piped: the reader below arrives on stdin itself, so stdin is
  # not free to carry data as well.
  xcrun simctl list devices available --json >"$json" 2>/dev/null || return 0

  python3 - "$json" <<'PY'
import json
import sys

for runtime, devices in json.load(open(sys.argv[1]))["devices"].items():
    if "iOS" not in runtime:
        continue
    version = runtime.rsplit(".", 1)[-1].replace("iOS-", "").replace("-", ".")
    for device in devices:
        if not device.get("isAvailable"):
            continue
        print("simulator\t%s\t%s\tiOS %s" % (device["udid"], device["name"], version))
PY
}

destinations="$workspace/destinations"
{ paired_devices; available_simulators; } >"$destinations"
count="$(wc -l <"$destinations" | tr -d ' ')"

if [ "$count" -eq 0 ]; then
  echo "No iOS destination found: no paired device is connected and no simulator is installed." >&2
  echo "Connect and trust a device, or install a simulator runtime in Xcode." >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# Which one
# ---------------------------------------------------------------------------

if [ -n "${DEVICE:-}" ]; then
  # By identifier or by name, so that neither a script nor a person has to look up a UDID.
  choice="$(awk -F'\t' -v want="$DEVICE" '$2 == want || $3 == want { print NR; exit }' "$destinations")"
  if [ -z "$choice" ]; then
    echo "No iOS destination matches DEVICE=$DEVICE. Run without DEVICE to see the list." >&2
    exit 1
  fi
else
  echo "Where should Almena be installed?"
  echo
  # No padded columns: a device name is whatever its owner typed, emoji and accents included,
  # and a width in bytes does not line those up in a terminal.
  awk -F'\t' '{ printf "  %2d)  %s — %s, %s\n", NR, $3, $1, $4 }' "$destinations"
  echo
  printf 'Number (1-%s): ' "$count"
  read -r choice

  case "$choice" in
    '' | *[!0-9]*)
      echo "That is not one of the numbers above." >&2
      exit 1
      ;;
  esac

  if [ "$choice" -lt 1 ] || [ "$choice" -gt "$count" ]; then
    echo "That is not one of the numbers above." >&2
    exit 1
  fi
fi

selected="$(sed -n "${choice}p" "$destinations")"
kind="$(printf '%s' "$selected" | cut -f1)"
identifier="$(printf '%s' "$selected" | cut -f2)"
name="$(printf '%s' "$selected" | cut -f3)"

# ---------------------------------------------------------------------------
# Build for that destination
# ---------------------------------------------------------------------------

if [ "$kind" = "device" ]; then
  target="aarch64"
elif [ "$(uname -m)" = "arm64" ]; then
  target="aarch64-sim"
else
  target="x86_64"
fi

echo
echo "Building for $name ($kind, $target)"
echo

# Output from a previous run is removed rather than reused. Tauri exports the bundle by
# renaming it into place and fails outright when a stale one is already sitting there — a
# deploy that stops because of last week's artefact is a deploy nobody trusts. The Rust build
# lives in `src-tauri/target` and is untouched, so this costs the export and nothing else. It
# is also what makes the bundle found below unambiguously the one just built.
rm -rf "$BUILD_DIR"

# Deliberately not `--debug`. That flag leaves the interface out of the bundle and points the
# application at the dev server on this computer instead, which is not something to install
# onto a phone.
pnpm tauri ios build --target "$target"

# The two destinations leave different things behind, and `devicectl` installs a `.app` —
# nothing else — so a device build has to be unpacked first.
#
#   simulator  build/<target>/Almena.app   ready to install
#   device     build/<target>/Almena.ipa   the exported, re-signed article
#
# The `.app` sitting inside the `.xcarchive` is deliberately not used: it is the application
# as it was before the export re-signed it for this destination.
ipa="$(find "$BUILD_DIR" -maxdepth 3 -name '*.ipa' -print -quit)"

if [ -n "$ipa" ]; then
  unzip -q "$ipa" -d "$workspace/ipa"
  app="$(find "$workspace/ipa/Payload" -maxdepth 1 -name '*.app' -print -quit)"
else
  app="$(find "$BUILD_DIR" -maxdepth 4 -name '*.app' -not -path '*.xcarchive/*' -print -quit)"
fi

if [ -z "$app" ]; then
  echo "The build produced neither a .app nor an .ipa under $BUILD_DIR." >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# Refuse to install something that would not work
# ---------------------------------------------------------------------------

# The interface is embedded in the executable, under the same hashed names Vite gave it. If
# the bundle Vite just wrote is not in there, this artefact has no interface of its own and
# would look for one over the network at whatever machine built it.
#
# `grep` reads the executable directly rather than taking `strings` down a pipe: `grep -q`
# stops at the first match, which leaves `strings` writing into a closed pipe, and under
# `pipefail` that turns a found bundle into a failed check.
bundle="$(basename "$(find dist/assets -name '*.js' -print -quit)")"
executable="$app/$(basename "$app" .app)"

if [ -z "$bundle" ] || ! grep -qa "/assets/$bundle" "$executable"; then
  echo "Refusing to install: $app carries no interface." >&2
  echo "The application would try to load it from this computer, and show nothing anywhere else." >&2
  echo "This is what a --debug build produces; the build above should not have used one." >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# Install
# ---------------------------------------------------------------------------

echo
echo "Installing $app on $name"

if [ "$kind" = "device" ]; then
  xcrun devicectl device install app --device "$identifier" "$app"
  echo "Installed. Open Almena on $name."
else
  # Booting an already-booted simulator is an error worth ignoring, and the window has to be
  # brought up separately: simctl installs onto a simulator whether or not anyone is looking.
  xcrun simctl boot "$identifier" 2>/dev/null || true
  open -a Simulator
  xcrun simctl install "$identifier" "$app"
  xcrun simctl launch "$identifier" "$BUNDLE_ID"
fi
