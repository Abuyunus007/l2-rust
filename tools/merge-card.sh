#!/usr/bin/env bash
# Orchestrator helper: merge a finished worker branch into main.
# Usage: tools/merge-card.sh <agent-id> <CARD-ID> [next CARD-IDs to mark in-progress...]  (full ids, e.g. M2-DB-02)
# Stops (exit 1) on any conflict other than Cargo.lock, or on failing checks.
set -uo pipefail
cd "$(dirname "$0")/.."
agent="$1"; card="$2"; shift 2
branch="worktree-agent-$agent"

if [ ! -f .git/MERGE_HEAD ]; then
  git merge --no-ff "$branch" -m "Merge $card

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" >/dev/null 2>&1
fi
for f in $(git diff --name-only --diff-filter=U); do
  case "$f" in
    Cargo.lock) git checkout --ours Cargo.lock ;;
    Cargo.toml|*/lib.rs|*/mod.rs|*/error.rs) python -I tools/union-conflict.py "$f" ;;   # parallel cards adding mod/use lines: keep both
    *) echo "CONFLICT in: $f (resolve by hand, then rerun this script)"; exit 1 ;;
  esac
  git add "$f"
done
if [ -f .git/MERGE_HEAD ]; then cargo fmt --all >/dev/null 2>&1; git add -u; git commit -q --no-edit; fi
cargo metadata --format-version 1 >/dev/null 2>&1   # refresh Cargo.lock if needed

cargo fmt --all -- --check >/dev/null 2>&1 || { echo "FMT FAILED"; exit 1; }
CARGO_INCREMENTAL=0 cargo clippy -q --workspace --all-targets -- -D warnings 2>&1 | grep -E "^(error|warning)" && { echo "CLIPPY FAILED"; exit 1; }
log=$(mktemp)
CARGO_INCREMENTAL=0 cargo test -q --workspace --no-fail-fast >"$log" 2>&1
summary=$(grep -E "test result" "$log" | awk '{s+=$4; f+=$6} END {print s" passed, "f" failed"}')
echo "tests: $summary"
case "$summary" in *" 0 failed") rm -f "$log" ;; *) grep -E "panicked at|^test .* FAILED|^error" "$log" | head -10; echo "TESTS FAILED (log: $log)"; exit 1 ;; esac

git worktree remove --force --force ".claude/worktrees/agent-$agent" 2>/dev/null
# The incremental cache of the main checkout only grows (it reached 62 GB): drop it after each merge.
rm -rf target/debug/incremental
git branch -q -D "$branch" 2>/dev/null

sed -i "s/| \(\[$card\][^|]*|[^|]*|[^|]*|[^|]*|[^|]*| \)[^|]* |/| \1merged |/" docs/tasks/BOARD.md
for next in "$@"; do
  case "$next" in M[0-9]*-*) ;; *) next="M1-$next" ;; esac   # short form = an M1 card
  sed -i "s/| \(\[$next\][^|]*|[^|]*|[^|]*|[^|]*|[^|]*| \)[^|]* |/| \1in-progress |/" docs/tasks/BOARD.md
done
git add -A docs/tasks/BOARD.md Cargo.lock
git commit -q -m "Board: $card merged${*:+, started $*}

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
git -c credential.helper= -c 'credential.helper=!gh auth git-credential' push -q
git log --oneline -1
