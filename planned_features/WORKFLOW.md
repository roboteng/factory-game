# Feature Planning Workflow

## Goal
Keep the game easy to change. Every step of this workflow serves that goal.

---

## The Process

### 1. Pick a feature
Choose from the feature files in this directory. Each file describes what to build and why.

### 2. Consider two implementations
Before writing any code, write down at least two ways to implement the feature.

For each option describe:
- **What it is** — a short technical summary
- **Best suited for** — the kind of situation or future direction where this option wins
- **Tradeoffs** — what you give up

### 3. Choose the most changeable one
By default, pick the option that is easiest to change later — not the most complete, not the most clever.
If the options are close, prefer less code and fewer abstractions.

### 4. Find enabling refactors
Before implementing, look for small refactors that make the chosen implementation cleaner or safer.
Do these first. They should be independent commits.

### 5. Implement
Make the change. Keep it scoped to what the feature actually needs.

### 6. Consider tests
Tests are optional. Write them when:
- The behavior is tricky enough that you might silently break it later
- The invariant is something you're confident should always hold

Avoid tests that:
- Lock in a specific internal structure (bad: tests asserting on Vec index layout)
- Would need to change when you refactor internals (coupling = fragile)

Good signal: if a test would still pass after a complete internal rewrite that preserves behavior, it's worth keeping.

---

## Feature File Format

Each feature gets its own markdown file in this directory:

```
planned_features/
  WORKFLOW.md          ← this file
  <feature-name>.md   ← one per feature
```

Feature files use this structure:

```markdown
# Feature Name

## What
One paragraph: what this adds to the game and why it matters.

## Implementations

### Option A: <name>
- **Summary:** ...
- **Best suited for:** ...
- **Tradeoffs:** ...

### Option B: <name>
- **Summary:** ...
- **Best suited for:** ...
- **Tradeoffs:** ...

## Chosen: Option <A|B>
Reason: ...

## Enabling Refactors
- [ ] ...

## Status
[ ] Planned / [ ] Refactoring / [ ] In Progress / [ ] Done
```
