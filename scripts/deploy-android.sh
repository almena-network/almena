#!/usr/bin/env bash
#
# Builds Almena for one Android destination and installs it there.
#
# The counterpart of `deploy-ios.sh`, and deliberately the same shape: list what this machine
# can install onto, let the person choose, build for that destination, check the artefact, and
# only then put it on the device. The two platforms are equals in this project and their
# deploys should not feel like two different tools.
#
# Android adds one step iOS does not need. A simulator that is not booted can be booted in a
# second; an emulator that is not running has to be started and waited for, and its ABI cannot
# be asked for until it is up. That is why the destination is settled, and started, before
# anything is compiled.
#
# Run from the project root, which is where Task runs it. Set DEVICE to skip the prompt.

set -euo pipefail

# The bundle identifier, which has to agree with `identifier` in
# `src-tauri/tauri.conf.json`. Still the scaffold's guess — see spec 0001.
readonly PACKAGE_ID="network.almena.desktop"
readonly GRADLE_OUTPUT="src-tauri/gen/android/app/build/outputs/apk"

workspace="$(mktemp -d)"
trap 'rm -rf "$workspace"' EXIT

# ---------------------------------------------------------------------------
# The JDK Gradle is going to be handed
# ---------------------------------------------------------------------------

# Checked before anything else, because the default path is the broken one. Gradle refuses a
# JDK newer than it knows about and says so in a message naming neither Gradle nor Java
# ("Unsupported class file major version 69"), and when JAVA_HOME is unset the Tauri CLI hands
# it the JDK bundled with Android Studio — which on a current install is newer than the Gradle
# this project pins. Left alone, that is forty seconds of compiling followed by a wall of
# output that says nothing useful.

# Gradle 8.14, which is what `src-tauri/gen/android` pins. Bump both together.
readonly NEWEST_SUPPORTED_JDK=24
readonly STUDIO_JDK="/Applications/Android Studio.app/Contents/jbr/Contents/Home"

jdk_home() {
  if [ -n "${JAVA_HOME:-}" ]; then
    printf '%s' "$JAVA_HOME"
  elif [ -x "$STUDIO_JDK/bin/java" ]; then
    # What the Tauri CLI falls back to, so it is what would really be used.
    printf '%s' "$STUDIO_JDK"
  fi
}

jdk="$(jdk_home)"

if [ -z "$jdk" ] || [ ! -x "$jdk/bin/java" ]; then
  echo "No JDK found. Set JAVA_HOME — see README.md#requirements." >&2
  exit 1
fi

# `java -version` writes to stderr, and quotes the version on its first line.
jdk_major="$("$jdk/bin/java" -version 2>&1 | awk -F'"' 'NR == 1 { split($2, v, "."); print v[1] }')"

case "$jdk_major" in
  '' | *[!0-9]*)
    # An unreadable version is not grounds for refusing to build. Gradle will say its piece.
    ;;
  *)
    if [ "$jdk_major" -gt "$NEWEST_SUPPORTED_JDK" ]; then
      echo "Gradle in this project supports JDK $NEWEST_SUPPORTED_JDK and older, and this is JDK $jdk_major:" >&2
      echo "  $jdk" >&2
      echo "Point JAVA_HOME at an older one. README.md#requirements asks for JDK 17." >&2
      exit 1
    fi
    ;;
esac

emulator_binary() {
  if [ -n "${ANDROID_HOME:-}" ] && [ -x "$ANDROID_HOME/emulator/emulator" ]; then
    printf '%s' "$ANDROID_HOME/emulator/emulator"
  else
    command -v emulator 2>/dev/null || true
  fi
}

# ---------------------------------------------------------------------------
# What this machine can install onto
# ---------------------------------------------------------------------------

# Each line is: kind, identifier, name, detail — separated by tabs. `device` and `emulator`
# are attached and ready; `avd` is an emulator that exists but is not running yet.

attached() {
  # `adb devices -l` prints a header, then one line per device, then a blank line. Anything
  # not in the `device` state — unauthorized, offline, still booting — is left out rather than
  # listed and refused later.
  adb devices -l 2>/dev/null | awk '
    NR == 1 { next }
    $2 != "device" { next }
    {
      serial = $1
      model = "unknown"
      for (i = 3; i <= NF; i++) {
        if ($i ~ /^model:/) { model = substr($i, 7); gsub(/_/, " ", model) }
      }
      kind = (serial ~ /^emulator-/) ? "emulator" : "device"
      printf "%s\t%s\t%s\t%s\n", kind, serial, model, serial
    }
  '
}

stopped_emulators() {
  local emulator running
  emulator="$(emulator_binary)"
  [ -n "$emulator" ] || return 0

  # An emulator already in the list above must not appear twice. Its AVD name is what it
  # reports as `avd_name`, which is only askable while it runs.
  running="$workspace/running-avds"
  : >"$running"
  adb devices 2>/dev/null | awk 'NR > 1 && $2 == "device" && $1 ~ /^emulator-/ { print $1 }' |
    while read -r serial; do
      adb -s "$serial" emu avd name 2>/dev/null | head -1 | tr -d '\r' >>"$running"
    done

  "$emulator" -list-avds 2>/dev/null | while read -r avd; do
    [ -n "$avd" ] || continue
    grep -qxF "$avd" "$running" && continue
    printf 'avd\t%s\t%s\tnot running\n' "$avd" "$avd"
  done
}

destinations="$workspace/destinations"
{ attached; stopped_emulators; } >"$destinations"
count="$(wc -l <"$destinations" | tr -d ' ')"

if [ "$count" -eq 0 ]; then
  echo "No Android destination found: no device is attached and no emulator is installed." >&2
  echo "Connect a device with USB debugging on, or create an emulator in Android Studio." >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# Which one
# ---------------------------------------------------------------------------

if [ -n "${DEVICE:-}" ]; then
  # By serial, by AVD name or by model, so that neither a script nor a person has to copy a
  # serial out of `adb devices`.
  choice="$(awk -F'\t' -v want="$DEVICE" '$2 == want || $3 == want { print NR; exit }' "$destinations")"
  if [ -z "$choice" ]; then
    echo "No Android destination matches DEVICE=$DEVICE. Run without DEVICE to see the list." >&2
    exit 1
  fi
else
  echo "Where should Almena be installed?"
  echo
  # No padded columns: a device model is whatever its maker wrote, and a width in bytes does
  # not line those up in a terminal.
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
# Start it, if it is not running
# ---------------------------------------------------------------------------

if [ "$kind" = "avd" ]; then
  echo
  echo "Starting $identifier"

  before="$workspace/serials-before"
  adb devices 2>/dev/null | awk 'NR > 1 && $1 != "" { print $1 }' >"$before"

  "$(emulator_binary)" -avd "$identifier" >"$workspace/emulator.log" 2>&1 &

  # The serial is not known in advance, so it is whichever one appears that was not there
  # before. Waiting for the process is not enough: an emulator answers adb well before the
  # system is up, and installing into a half-booted one fails in ways that read as our bug.
  serial=""
  waited=0
  while [ -z "$serial" ] || [ "$(adb -s "$serial" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" != "1" ]; do
    if [ "$waited" -ge 300 ]; then
      echo "The emulator did not finish starting within five minutes." >&2
      echo "Its output is in $workspace/emulator.log — which this script removes on exit." >&2
      exit 1
    fi
    sleep 2
    waited=$((waited + 2))
    if [ -z "$serial" ]; then
      serial="$(adb devices 2>/dev/null |
        awk 'NR > 1 && $1 ~ /^emulator-/ { print $1 }' |
        grep -vxF -f "$before" | head -1 || true)"
    fi
  done

  identifier="$serial"
  echo "Started as $identifier"
fi

# ---------------------------------------------------------------------------
# Build for that destination
# ---------------------------------------------------------------------------

# The destination's own ABI, asked of the destination rather than assumed. Building the one
# that is going to be installed is the whole reason the choice comes first.
abi="$(adb -s "$identifier" shell getprop ro.product.cpu.abi 2>/dev/null | tr -d '\r')"

case "$abi" in
  arm64-v8a) target="aarch64" ;;
  armeabi-v7a) target="armv7" ;;
  x86_64) target="x86_64" ;;
  x86) target="i686" ;;
  *)
    echo "The destination reports an ABI this project does not build for: ${abi:-none}." >&2
    exit 1
    ;;
esac

echo
echo "Building for $name ($kind, $abi)"
echo

# Output from a previous run is removed rather than reused, so that the package found below is
# unambiguously the one just built. The Rust build lives in `src-tauri/target` and is
# untouched, so this costs the packaging step and nothing else.
rm -rf "$GRADLE_OUTPUT"

# `--debug` is what makes the package installable without a release keystore, which is the
# whole point of a deploy: this is the application on the device on your desk, not an artefact
# for distribution. `task build:android` is the other one. What `--debug` must not cost is the
# interface, and the check below is what makes sure it has not.
if ! pnpm tauri android build --debug --apk --target "$target"; then
  echo >&2
  echo "The Android build failed." >&2
  echo "If the failure above mentions \"class file major version\", that is Gradle refusing the" >&2
  echo "JDK it was handed: it names neither Java nor Gradle, and it means the JDK is newer than" >&2
  echo "the Gradle in src-tauri/gen/android supports. Point JAVA_HOME at a supported one — see" >&2
  echo "README.md#requirements." >&2
  exit 1
fi

apk="$(find "$GRADLE_OUTPUT" -name '*.apk' -print -quit 2>/dev/null || true)"

if [ -z "$apk" ]; then
  echo "The build produced no .apk under $GRADLE_OUTPUT." >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# Refuse to install something that would not work
# ---------------------------------------------------------------------------

# The bundle Vite just wrote has to be somewhere inside the package — in its assets or in the
# native library, depending on how the build embedded it. If it is nowhere, this artefact has
# no interface of its own and would look for one over the network at the machine that built
# it: it would work on this desk and show nothing on anyone else's.
bundle="$(basename "$(find dist/assets -name '*.js' -print -quit)")"
unzip -q "$apk" -d "$workspace/apk"

if [ -z "$bundle" ] || ! grep -rqa "$bundle" "$workspace/apk"; then
  echo "Refusing to install: $apk carries no interface." >&2
  echo "The application would try to load it from this computer, and show nothing anywhere else." >&2
  echo "Building without --debug embeds it, but then the package needs a release keystore." >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# Install
# ---------------------------------------------------------------------------

echo
echo "Installing $apk on $name"

adb -s "$identifier" install -r "$apk"
adb -s "$identifier" shell monkey -p "$PACKAGE_ID" -c android.intent.category.LAUNCHER 1 >/dev/null 2>&1 ||
  echo "Installed. Open Almena on $name."
