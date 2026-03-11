#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
allowlist_path="${repo_root}/scripts/inline_rust_test_allowlist.txt"

if ! command -v rg >/dev/null 2>&1; then
  echo "check_inline_rust_tests.sh requires 'rg' in PATH" >&2
  exit 1
fi

mapfile -t allowlist < <(grep -Ev '^\s*(#|$)' "${allowlist_path}" | sed 's#^\./##' | sort -u)
mapfile -t detected < <(
  cd "${repo_root}"
  find bins crates -type f -name '*.rs' -path '*/src/*' 2>/dev/null \
    | grep -v '/src/tests/' \
    | while read -r path; do
        if rg -q '#\[cfg\(test\)\]|#\[test\]|tokio::test' "${path}"; then
          printf '%s\n' "${path}"
        fi
      done \
    | sort -u
)
mapfile -t unexpected < <(
  comm -23 \
    <(printf '%s\n' "${detected[@]}") \
    <(printf '%s\n' "${allowlist[@]}")
)

if ((${#unexpected[@]} > 0)); then
  echo "Unexpected inline Rust tests in production src/ files:" >&2
  printf '  %s\n' "${unexpected[@]}" >&2
  echo >&2
  echo "Move the test to tests/ by default, or add an allowlist entry if private-item access is required." >&2
  exit 1
fi

echo "Inline Rust test guard passed."
