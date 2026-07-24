#!/bin/sh
# Release gate for the tanzim workspace. Run this right before tagging a release
# (the `crate*.*.*` tag): it blocks the release unless the working tree is
# self-consistent and correctly versioned relative to the last release.
#
# Two rules are enforced together, and between them they cover the whole bump
# cascade (change a leaf crate -> it must bump -> every dependent must repoint
# and therefore bump -> up to the `tanzim` facade):
#
#   1. Every internal `tanzim-*` dependency pins the *current* version of the
#      crate it points at. So if a crate is bumped, each dependent is forced to
#      edit its Cargo.toml to the new version.
#   2. Every crate whose published inputs (src/, Cargo.toml, README.md) changed
#      since the last release tag must have a different version than it had at
#      that tag. Because rule 1 turns a dependency bump into a Cargo.toml edit,
#      that edit trips this rule for the dependent, and the requirement cascades
#      automatically all the way up.

set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname "$0")" && pwd)

for cmd in awk git; do
	if ! command -v "$cmd" >/dev/null 2>&1; then
		echo "error: required command not found: $cmd" >&2
		exit 1
	fi
done

# Internal crates (leaves first, facade last). The CLI (`tanzim-cli`, tagged
# separately as `cli*`) is intentionally out of scope.
CRATES="
tanzim-value
tanzim-source
tanzim-load
tanzim-parse
tanzim-merge
tanzim-validate
tanzim-testing
tanzim
"

# Print the [package] version of a Cargo.toml fed on stdin. Reused throughout,
# both for on-disk manifests and for `git show`n manifests at the baseline tag.
read_pkg_version() {
	awk '
		/^\[package\]/ { in_pkg = 1; next }
		/^\[/ { in_pkg = 0 }
		in_pkg && /^version = "/ {
			line = $0
			sub(/^version = "/, "", line)
			sub(/".*$/, "", line)
			print line
			exit
		}
	'
}

case "${1-}" in
--help | -h)
	cat <<EOF
Usage: $(basename "$0") [--check]

Release gate. Verifies that internal dependency versions are consistent and
that every crate changed since the last release tag has been bumped. Run it
before tagging a release; a non-zero exit means the release must not proceed.
EOF
	exit 0
	;;
--check | "")
	: # the one and only mode
	;;
*)
	echo "error: unknown command: $1 (try --help)" >&2
	exit 1
	;;
esac

errors=0

# --- Collect the current on-disk version of every crate ---------------------
versions_file=$(mktemp)
trap 'rm -f "$versions_file"' EXIT INT HUP TERM

for crate in $CRATES; do
	toml="$SCRIPT_DIR/$crate/Cargo.toml"
	if [ ! -f "$toml" ]; then
		echo "error: missing Cargo.toml for crate $crate: $toml" >&2
		errors=1
		continue
	fi
	version=$(read_pkg_version <"$toml")
	if [ -z "$version" ]; then
		echo "error: could not read package version from $toml" >&2
		errors=1
		continue
	fi
	printf '%s %s\n' "$crate" "$version" >>"$versions_file"
done

# --- Rule 1: every internal dependency pins the crate's current version ------
for crate in $CRATES; do
	toml="$SCRIPT_DIR/$crate/Cargo.toml"
	[ -f "$toml" ] || continue

	# Walk [dependencies]/[dev-dependencies], and for each `tanzim-* = { ...
	# version = "x.y.z" ... }` line compare the pinned version to the current
	# version of that crate (loaded above into versions_file).
	awk -v toml="$toml" -v versions_file="$versions_file" '
		BEGIN {
			while ((getline line < versions_file) > 0) {
				split(line, parts, " ")
				latest[parts[1]] = parts[2]
			}
			close(versions_file)
			in_deps = 0
		}
		/^\[(dependencies|dev-dependencies)\]/ { in_deps = 1; next }
		/^\[/ { in_deps = 0 }
		in_deps && /^tanzim-/ {
			eq = index($0, " =")
			if (eq == 0) { next }
			dep = substr($0, 1, eq - 1)
			if (match($0, /version = "[0-9]+\.[0-9]+\.[0-9]+"/)) {
				used = substr($0, RSTART, RLENGTH)
				sub(/^version = "/, "", used)
				sub(/"$/, "", used)
				if (!(dep in latest)) {
					printf "error: %s: unknown internal dependency %s\n", toml, dep > "/dev/stderr"
					exit 2
				}
				if (used != latest[dep]) {
					printf "error: %s: %s pinned at \"%s\" but current version is \"%s\"\n", toml, dep, used, latest[dep] > "/dev/stderr"
					exit 2
				}
			}
		}
	' "$toml" || errors=1
done

# --- Rule 2: crates changed since the last release must be bumped ------------
# Baseline = newest `crate*` tag that is not the current HEAD, so the gate still
# works when re-run on a commit that already carries the release tag.
baseline=""
head_rev=$(git rev-parse HEAD 2>/dev/null || true)
for tag in $(git tag --list 'crate*' --sort=-version:refname 2>/dev/null); do
	if [ "$(git rev-list -n1 "$tag" 2>/dev/null)" = "$head_rev" ]; then
		continue
	fi
	baseline="$tag"
	break
done

if [ -z "$baseline" ]; then
	echo "warning: no prior 'crate*' release tag found; skipping changed-since-release check." >&2
else
	for crate in $CRATES; do
		# "Changed" means any published input differs from the baseline tag.
		if git diff --quiet "$baseline" -- \
			"crates/$crate/src" \
			"crates/$crate/Cargo.toml" \
			"crates/$crate/README.md" 2>/dev/null; then
			continue
		fi
		current=$(awk -v c="$crate" '$1 == c { print $2 }' "$versions_file")
		old=$(git show "$baseline:crates/$crate/Cargo.toml" 2>/dev/null | read_pkg_version)
		if [ -z "$old" ]; then
			echo "note: $crate did not exist at $baseline; treating as new (no bump required)." >&2
			continue
		fi
		if [ "$current" = "$old" ]; then
			echo "error: $crate changed since $baseline but its version is still $current (bump required)" >&2
			errors=1
		fi
	done
fi

rm -f "$versions_file"
trap - EXIT INT HUP TERM

if [ "$errors" -ne 0 ]; then
	echo "Release gate failed." >&2
	exit 1
fi

echo "Release gate passed: internal versions consistent and all crates changed since ${baseline:-<no baseline>} are bumped."
