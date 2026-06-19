#!/usr/bin/env bash
# Sync local branch with origin without relying on unset pull.rebase / pull.ff.
set -euo pipefail

branch="$(git branch --show-current)"
remote="${1:-origin}"

git fetch "$remote"
git pull --ff-only "$remote" "$branch"
