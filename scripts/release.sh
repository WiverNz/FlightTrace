#!/usr/bin/env bash
# release.sh — bump the FlightTrace version, commit, and tag.
#
# Usage:
#   scripts/release.sh                  # auto-bump patch:   0.1.3 → 0.1.4
#   scripts/release.sh patch            # same as above
#   scripts/release.sh minor            # 0.1.3 → 0.2.0
#   scripts/release.sh major            # 0.1.3 → 1.0.0
#   scripts/release.sh 1.2.3            # explicit version
#   scripts/release.sh 1.2.3 -m "notes" # custom tag message
#
# Flags:
#   -m, --message <text>   Annotated tag message (default: "Release vX.Y.Z").
#   -n, --dry-run          Print the plan, change nothing.
#   -y, --yes              Skip the confirmation prompt.
#       --no-sign          Do not sign the tag (default: sign if user.signingKey is set).
#       --push             Push the branch and tag after tagging. Otherwise print the commands.
#   -h, --help             Show this help.
#
# What it does:
#   1. Requires a clean git working tree on a branch.
#   2. Reads the current version from workspace Cargo.toml.
#   3. Computes a SemVer bump or uses an explicit version.
#   4. Updates Cargo.toml, web/package.json, and web/package-lock.json.
#   5. Runs cargo check --workspace so Cargo.lock catches up.
#   6. Shows the diff and creates one "Release vX.Y.Z" commit.
#   7. Creates annotated tag vX.Y.Z and optionally pushes it.
#   8. A pushed tag triggers CI to build and publish the GitHub Release.

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

cargo_toml="Cargo.toml"
cargo_lock="Cargo.lock"
web_package="web/package.json"
web_lock="web/package-lock.json"
desktop_cargo_toml="apps/flighttrace-desktop/src-tauri/Cargo.toml"
desktop_cargo_lock="apps/flighttrace-desktop/src-tauri/Cargo.lock"
desktop_tauri_conf="apps/flighttrace-desktop/src-tauri/tauri.conf.json"

# ---------- argument parsing ----------

version_spec=""
message=""
dry_run=0
assume_yes=0
no_sign=0
do_push=0

usage() { sed -n '2,28{s/^# \{0,1\}//;p;}' "$0"; }
die() { echo "error: $*" >&2; exit 1; }

while (( $# )); do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    -n|--dry-run)
      dry_run=1
      shift
      ;;
    -y|--yes)
      assume_yes=1
      shift
      ;;
    --no-sign)
      no_sign=1
      shift
      ;;
    --push)
      do_push=1
      shift
      ;;
    -m|--message)
      (( $# >= 2 )) || die "$1 requires a value"
      message="$2"
      shift 2
      ;;
    --)
      shift
      break
      ;;
    -*)
      die "unknown option: $1"
      ;;
    *)
      [[ -z "$version_spec" ]] || die "multiple version arguments supplied"
      version_spec="$1"
      shift
      ;;
  esac
done

(( $# == 0 )) || die "unexpected argument: $1"

# ---------- helpers ----------

require_clean_tree() {
  git rev-parse --is-inside-work-tree >/dev/null 2>&1 \
    || die "not inside a git working tree"
  if [[ -n "$(git status --porcelain)" ]]; then
    git status --short
    die "working tree is dirty — commit or stash first"
  fi
  [[ -n "$(git symbolic-ref --short -q HEAD || true)" ]] \
    || die "not on a branch (detached HEAD)"
}

current_version() {
  awk '
    /^\[workspace\.package\]/ { in_section = 1; next }
    /^\[/                      { in_section = 0 }
    in_section && /^version *= *"/ {
      match($0, /"[^"]*"/)
      print substr($0, RSTART + 1, RLENGTH - 2)
      exit
    }
  ' "$cargo_toml"
}

web_version() {
  node -p "require('./$web_package').version"
}

valid_semver() {
  [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$ ]]
}

bump_version() {
  local current="$1"
  local kind="$2"
  local major minor patch
  IFS=. read -r major minor patch <<<"${current%%[+-]*}"
  case "$kind" in
    major) echo "$((major + 1)).0.0" ;;
    minor) echo "${major}.$((minor + 1)).0" ;;
    patch) echo "${major}.${minor}.$((patch + 1))" ;;
    *) die "internal: unsupported bump kind: $kind" ;;
  esac
}

write_workspace_version() {
  local new="$1"
  local temporary
  temporary="$(mktemp "${cargo_toml}.XXXXXX")"
  awk -v new="$new" '
    BEGIN { in_section = 0; replaced = 0 }
    /^\[workspace\.package\]/ { in_section = 1; print; next }
    /^\[/                      { in_section = 0 }
    {
      if (in_section && !replaced && $0 ~ /^version *= *"/) {
        print "version = \"" new "\""
        replaced = 1
      } else {
        print
      }
    }
    END {
      if (!replaced) exit 1
    }
  ' "$cargo_toml" > "$temporary" || {
    rm -f "$temporary"
    die "could not update [workspace.package].version"
  }
  mv "$temporary" "$cargo_toml"
}

# Desktop crate is excluded from the workspace (see apps/flighttrace-desktop
# README + docs/issues/09), so its Cargo.toml carries a standalone
# `version = "..."` line under `[package]`. Bump it in lockstep.
write_desktop_cargo_version() {
  local new="$1"
  local temporary
  temporary="$(mktemp "${desktop_cargo_toml}.XXXXXX")"
  awk -v new="$new" '
    BEGIN { in_section = 0; replaced = 0 }
    /^\[package\]/ { in_section = 1; print; next }
    /^\[/          { in_section = 0 }
    {
      if (in_section && !replaced && $0 ~ /^version *= *"/) {
        print "version = \"" new "\""
        replaced = 1
      } else {
        print
      }
    }
    END {
      if (!replaced) exit 1
    }
  ' "$desktop_cargo_toml" > "$temporary" || {
    rm -f "$temporary"
    die "could not update [package].version in $desktop_cargo_toml"
  }
  mv "$temporary" "$desktop_cargo_toml"
}

# tauri.conf.json's top-level `"version"` field. Uses a small Python
# oneliner because sed on JSON is error-prone.
write_tauri_conf_version() {
  local new="$1"
  python3 - "$desktop_tauri_conf" "$new" <<'PY'
import json, sys
path, new = sys.argv[1], sys.argv[2]
with open(path, "r", encoding="utf-8") as f:
    data = json.load(f)
data["version"] = new
with open(path, "w", encoding="utf-8") as f:
    json.dump(data, f, indent=2)
    f.write("\n")
PY
}

write_versions() {
  local new="$1"
  write_workspace_version "$new"
  write_desktop_cargo_version "$new"
  write_tauri_conf_version "$new"
  npm --prefix web version "$new" --no-git-tag-version --allow-same-version >/dev/null
}

# ---------- main ----------

require_clean_tree

current="$(current_version)"
[[ -n "$current" ]] || die "could not read current version from $cargo_toml"
valid_semver "$current" || die "current version is not valid SemVer: $current"

current_web="$(web_version)"
[[ "$current_web" == "$current" ]] \
  || die "$web_package version ($current_web) does not match workspace version ($current)"

# Desktop crate has its own [package] version (excluded from workspace).
current_desktop_cargo="$(awk '
  /^\[package\]/ { in_section = 1; next }
  /^\[/          { in_section = 0 }
  in_section && /^version *= *"/ {
    match($0, /"[^"]*"/)
    print substr($0, RSTART + 1, RLENGTH - 2)
    exit
  }
' "$desktop_cargo_toml")"
[[ "$current_desktop_cargo" == "$current" ]] \
  || die "$desktop_cargo_toml version ($current_desktop_cargo) does not match workspace version ($current)"

current_tauri="$(python3 -c "import json,sys; print(json.load(open('$desktop_tauri_conf'))['version'])")"
[[ "$current_tauri" == "$current" ]] \
  || die "$desktop_tauri_conf version ($current_tauri) does not match workspace version ($current)"

case "${version_spec:-patch}" in
  patch|minor|major)
    new="$(bump_version "$current" "${version_spec:-patch}")"
    ;;
  *)
    new="$version_spec"
    ;;
esac

valid_semver "$new" || die "not a valid SemVer: $new"
[[ "$new" != "$current" ]] \
  || die "new version equals current version ($current)"
[[ -n "$message" ]] || message="Release v$new"

if git rev-parse --quiet --verify "refs/tags/v$new" >/dev/null; then
  die "tag v$new already exists"
fi

cat <<EOF
Plan:
  current version : $current
  new version     : $new
  files to update : $cargo_toml
                    $cargo_lock
                    $desktop_cargo_toml
                    $desktop_cargo_lock (if it exists)
                    $desktop_tauri_conf
                    $web_package
                    $web_lock
  tag             : v$new
  tag message     : $message
EOF

if (( dry_run )); then
  echo "(dry-run — exiting before any changes)"
  exit 0
fi

if (( ! assume_yes )); then
  read -r -p "proceed? [y/N] " answer
  [[ "$answer" =~ ^[Yy]$ ]] || {
    echo "aborted."
    exit 1
  }
fi

write_versions "$new"

echo "→ cargo check --workspace (refreshes Cargo.lock)"
cargo check --workspace --quiet

# Refresh the desktop crate's Cargo.lock too if it exists (only present
# when someone has already run `cargo build` inside src-tauri/).
if [[ -f "$desktop_cargo_lock" ]]; then
  echo "→ cargo check (desktop crate, refreshes its Cargo.lock)"
  (cd apps/flighttrace-desktop/src-tauri && cargo check --quiet) || \
    echo "warning: desktop cargo check failed — Cargo.lock may be stale until the next build" >&2
fi

# Only stage files that actually exist so a first-time desktop-lock
# absence doesn't break the commit.
staged=("$cargo_toml" "$cargo_lock" "$desktop_cargo_toml" "$desktop_tauri_conf" "$web_package" "$web_lock")
[[ -f "$desktop_cargo_lock" ]] && staged+=("$desktop_cargo_lock")

git --no-pager diff --stat -- "${staged[@]}"
echo

git add -- "${staged[@]}"
git commit -m "Release v$new" >/dev/null
echo "→ committed: $(git rev-parse --short HEAD)  Release v$new"

signed=0
if (( ! no_sign )) && git config --get user.signingKey >/dev/null 2>&1; then
  signed=1
fi

if (( signed )); then
  git tag -s "v$new" -m "$message"
else
  git tag -a "v$new" -m "$message"
fi
echo "→ tagged: v$new$([[ $signed == 1 ]] && echo ' (signed)')"

branch="$(git symbolic-ref --short HEAD)"
if (( do_push )); then
  echo "→ git push origin $branch"
  git push origin "$branch"
  echo "→ git push origin v$new"
  git push origin "v$new"
  echo "→ GitHub Actions will build and publish Release v$new"
else
  cat <<EOF

Push when ready:
  git push origin $branch
  git push origin v$new

Or rerun with --push on a future release.
Pushing the tag triggers the GitHub Release workflow.
EOF
fi
