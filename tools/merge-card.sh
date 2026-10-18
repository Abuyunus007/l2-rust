#!/usr/bin/env bash
# Orchestrator helper: merge a finished worker branch into main.
# Usage: tools/merge-card.sh <agent-id> <CARD-ID> [next-card-ids-to-mark-in-progress...]
# Stops (exit 1) on any conflict other than Cargo.lock, or on failing checks.
set -uo pipefail
cd "$(dirname "$0")/.."
agent="$1"; card="$2"; shift 2
branch="worktree-agent-$agent"

git merge --no-ff "$branch" -m "Merge $card

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" >/dev/null 2>&1
conflicts=$(git diff --name-only --diff-filter=U)
if [ -n "$conflicts" ]; then
  if [ "$conflicts" = "Cargo.lock" ]; then
    git checkout --ours Cargo.lock && git add Cargo.lock && git commit -q --no-edit
  else
    echo "CONFLICT in: $conflicts (resolve by hand, then rerun checks)"; exit 1
  fi
fi
cargo metadata --format-version 1 >/dev/null 2>&1   # refresh Cargo.lock if needed

cargo fmt --all -- --check >/dev/null 2>&1 || { echo "FMT FAILED"; exit 1; }
cargo clippy -q --workspace --all-targets -- -D warnings 2>&1 | grep -E "^(error|warning)" && { echo "CLIPPY FAILED"; exit 1; }
summary=$(cargo test -q --workspace 2>&1 | grep -E "test result" | awk '{s+=$4; f+=$6} END {print s" passed, "f" failed"}')
echo "tests: $summary"
case "$summary" in *" 0 failed") ;; *) echo "TESTS FAILED"; exit 1 ;; esac

git worktree remove --force --force ".claude/worktrees/agent-$agent" 2>/dev/null
git branch -q -D "$branch" 2>/dev/null

re_card="${card#M1-}"
sed -i "s/| \(\[M1-$re_card\][^|]*|[^|]*|[^|]*|[^|]*|[^|]*| \)[a-z-]* |/| \1merged |/" docs/tasks/BOARD.md
for next in "$@"; do
  sed -i "s/| \(\[M1-$next\][^|]*|[^|]*|[^|]*|[^|]*|[^|]*| \)[a-z-]* |/| \1in-progress |/" docs/tasks/BOARD.md
done
git add -A docs/tasks/BOARD.md Cargo.lock
git commit -q -m "Board: $card merged${*:+, started $*}

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
git -c credential.helper= -c 'credential.helper=!gh auth git-credential' push -q
git log --oneline -1
