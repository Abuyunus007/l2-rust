# Task workflow

How work moves from the plan to `main`. Roles:
- **Orchestrator** = the Opus session you talk to.
- **Workers** = `porter-haiku`, `porter-sonnet` (`.claude/agents/`).
- **Reviewer** = `reviewer`.

## Lifecycle
```
 planned ──► ready ──► in-progress ──► review ──► merged
                ▲            │             │
                └── blocked ◄┘     fix-needed (back to the worker)
```
1. **Plan:** at the start of a milestone the orchestrator splits its scope (`docs/PLAN.md`) into cards from `TEMPLATE.md` under `docs/tasks/M<n>/` and lists them in `BOARD.md`.
2. **Ready:** a card is ready when its dependencies are merged and its reference implementation exists.
3. **Dispatch:** the orchestrator starts the worker named in the card's `Model` field with `isolation: worktree`, so each card gets its own branch. Independent ready cards run in parallel (max throughput, D-40). Cards touching the same files never run at the same time. Exception: cards that only add a `mod`/re-export line to a shared `lib.rs` (or a member to a manifest) may run in parallel, and the orchestrator resolves those one-line merge conflicts.
4. **Report:** the worker ends with its report (done / blocked).
5. **Review:** `Review: full` → the reviewer agent does a full review. `Review: spot` → automatic checks + the reviewer on a sample (D-44).
6. **Merge:** the orchestrator merges into `main` after a `merge` verdict and green CI, then updates `BOARD.md`.
7. **Blocked / fix-needed:** the orchestrator fixes the card (clearer spec, split, upgrade the model haiku → sonnet → opus) and dispatches again. A card that fails twice on Haiku moves to Sonnet.

## Choosing the model for a card
| Card type | Model |
|---|---|
| Architecture, world loop, script API, first instance of a core pattern | opus (the orchestrator does it itself) |
| First instance of a non-core pattern ("Reference: yes"), DB repositories, geodata, formulas, state machines | sonnet |
| Copy of an existing pattern: packet structs, XML loaders, config structs, effect handlers, quests, admin commands, GUI views | haiku |

## Card size
Small enough for one agent session: one packet group, one loader, one handler family, one quest. Max ~800 changed lines. Bigger → split.

## Milestone gate
A milestone ends with the in-game/GUI test in `docs/PLAN.md`, done by you. Its results go into `BOARD.md` under the milestone.
