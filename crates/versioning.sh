#!/bin/sh
# This script verifies that all internal crate dependencies use the latest version.
# Additionally, it verifies that every crate that depends on a bumped crate is also bumped.

set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname "$0")" && pwd)

for cmd in awk; do
	if ! command -v "$cmd" >/dev/null 2>&1; then
		echo "error: required command not found: $cmd" >&2
		exit 1
	fi
done

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

show_help() {
	cat <<EOF
Usage: $(basename "$0") <command>

Commands:
  --check        Verify internal crate dependencies use exact latest versions
  --check-bump   Verify every crate that depends on a bumped crate is also bumped
  --help         Show this help message
EOF
}

run_check() {
	versions_file=$(mktemp)
	trap 'rm -f "$versions_file"' EXIT INT HUP TERM

	errors=0

	for crate in $CRATES; do
		toml="$SCRIPT_DIR/$crate/Cargo.toml"
		if [ ! -f "$toml" ]; then
			echo "error: missing Cargo.toml for crate $crate: $toml" >&2
			errors=1
			continue
		fi

		version=$(awk '
			/^\[package\]/ { in_pkg = 1; next }
			/^\[/ { in_pkg = 0 }
			in_pkg && /^version = "/ {
				line = $0
				sub(/^version = "/, "", line)
				sub(/".*$/, "", line)
				print line
				exit
			}
		' "$toml")

		if [ -z "$version" ]; then
			echo "error: could not read package version from $toml" >&2
			errors=1
			continue
		fi

		printf '%s %s\n' "$crate" "$version" >>"$versions_file"
	done

	for crate in $CRATES; do
		toml="$SCRIPT_DIR/$crate/Cargo.toml"
		if [ ! -f "$toml" ]; then
			continue
		fi

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
				if (eq == 0) {
					next
				}
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
						printf "error: %s: %s version \"%s\" != latest \"%s\"\n", toml, dep, used, latest[dep] > "/dev/stderr"
						exit 2
					}
				}
			}
		' "$toml" || errors=1
	done

	rm -f "$versions_file"
	trap - EXIT INT HUP TERM

	if [ "$errors" -ne 0 ]; then
		exit 1
	fi

	echo "All internal crate versions match latest."
}

run_check_bump() {
	bumped=""
	for toml in $(git diff --name-only HEAD~1 -- '*/Cargo.toml'); do
		old_ver=$(git show HEAD~1:"$toml" 2>/dev/null | awk '
			/^\[package\]/ { in_pkg = 1; next }
			/^\[/ { in_pkg = 0 }
			in_pkg && /^version = "/ {
				line = $0
				sub(/^version = "/, "", line)
				sub(/".*$/, "", line)
				print line
				exit
			}
		')
		new_ver=$(awk '
			/^\[package\]/ { in_pkg = 1; next }
			/^\[/ { in_pkg = 0 }
			in_pkg && /^version = "/ {
				line = $0
				sub(/^version = "/, "", line)
				sub(/".*$/, "", line)
				print line
				exit
			}
		' "$toml")
		[ -z "$old_ver" ] && continue
		[ "$old_ver" = "$new_ver" ] && continue
		crate=$(awk '
			/^\[package\]/ { in_pkg = 1; next }
			/^\[/ { in_pkg = 0 }
			in_pkg && /^name = "/ {
				line = $0
				sub(/^name = "/, "", line)
				sub(/".*$/, "", line)
				print line
				exit
			}
		' "$toml")
		[ -n "$crate" ] && bumped="$bumped $crate"
	done

	if [ -z "$bumped" ]; then
		echo "No crates bumped; nothing to check."
		return
	fi

	errors=0
	for dep in $bumped; do
		for crate in $CRATES; do
			toml="$SCRIPT_DIR/$crate/Cargo.toml"
			[ -f "$toml" ] || continue
			awk -v dep="$dep" '
				/^\[(dependencies|dev-dependencies)\]/ { in_deps = 1; next }
				/^\[/ { in_deps = 0 }
				in_deps {
					eq = index($0, " =")
					if (eq > 0 && substr($0, 1, eq - 1) == dep) { found = 1; exit }
				}
				END { exit !found }
			' "$toml" || continue
			old_ver=$(git show HEAD~1:"$toml" 2>/dev/null | awk '
				/^\[package\]/ { in_pkg = 1; next }
				/^\[/ { in_pkg = 0 }
				in_pkg && /^version = "/ {
					line = $0
					sub(/^version = "/, "", line)
					sub(/".*$/, "", line)
					print line
					exit
				}
			')
			new_ver=$(awk '
				/^\[package\]/ { in_pkg = 1; next }
				/^\[/ { in_pkg = 0 }
				in_pkg && /^version = "/ {
					line = $0
					sub(/^version = "/, "", line)
					sub(/".*$/, "", line)
					print line
					exit
				}
			' "$toml")
			if [ -n "$old_ver" ] && [ "$old_ver" = "$new_ver" ]; then
				echo "error: $crate depends on bumped $dep but $crate version was not bumped" >&2
				errors=1
			fi
		done
	done

	if [ "$errors" -ne 0 ]; then
		exit 1
	fi

	echo "All dependents of bumped crates were bumped."
}

case "${1-}" in
--help | -h)
	show_help
	;;
--check)
	run_check
	;;
--check-bump)
	run_check_bump
	;;
"")
	show_help
	exit 1
	;;
*)
	echo "error: unknown command: $1 (try --help)" >&2
	show_help
	exit 1
	;;
esac
