#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

export CARGO_INCREMENTAL=1
export HERDR_FORK_ID="${HERDR_FORK_ID:-tingxueren/herdr}"
if [[ -z "${HERDR_FORK_REVISION:-}" ]]; then
    revision=$(git rev-parse --short=12 HEAD)
    if [[ -n "$(git status --porcelain --untracked-files=normal)" ]]; then
        # Include new files too; different uncommitted builds must be identifiable.
        tree=$(
            {
                git diff --binary HEAD
                while IFS= read -r -d '' source_file; do
                    printf '\0%s\0' "$source_file"
                    git hash-object -- "$source_file"
                done < <(git ls-files -z --others --exclude-standard)
            } | git hash-object --stdin
        )
        revision="$revision.dirty.${tree:0:12}"
    fi
    export HERDR_FORK_REVISION="$revision"
fi
exec cargo build --release --locked --timings
