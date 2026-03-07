---
name: codspeed-profiling
description: "Use when diagnosing CodSpeed benchmark regressions, profiling with valgrind-codspeed/callgrind, comparing instruction counts between branches, or reading callgrind_annotate output. Also use when a CodSpeed PR check shows performance regression and you need to find the cause. Covers: just profile, callgrind Ir/Dr/Dw counts, flat vs tree annotation, branch comparison methodology."
---

# CodSpeed Profiling

Profile locally with the same valgrind-codspeed fork that CodSpeed CI uses, so instruction counts match what CI reports.

## Prerequisites

- `valgrind-codspeed` fork installed (check: `valgrind --version` shows `codspeed`)
- Project has a `just profile <bench> [filter]` recipe (or equivalent callgrind workflow)

## Diagnosing a Regression

### 1. Profile the regression

```bash
just profile <bench_name> <filter>
```

This produces three files:
- `profiles/<label>.callgrind` — raw callgrind data
- `profiles/<label>.callgrind.flat.txt` — flat annotation (self cost per function)
- `profiles/<label>.callgrind.tree.txt` — call-tree annotation (inclusive cost)

### 2. Profile the baseline for comparison

```bash
# Save the regression profile
cp profiles/<label>.callgrind.flat.txt profiles/<label>-regression.flat.txt

# Profile main
git stash && git checkout main
just profile <bench_name> <filter>
cp profiles/<label>.callgrind.flat.txt profiles/<label>-main.flat.txt

# Back to the branch
git checkout <branch> && git stash pop
```

### 3. Compare instruction counts

Filter for your hot-path functions and diff:

```bash
echo "=== MAIN ===" && rg 'your::module' profiles/<label>-main.flat.txt
echo "=== BRANCH ===" && rg 'your::module' profiles/<label>-regression.flat.txt
```

Focus on the **Ir** column (first number) — instruction count. This is what CodSpeed costs.

### 4. Get source-level annotation

```bash
callgrind_annotate --auto=yes profiles/<label>.callgrind 2>/dev/null \
  | rg -A 5 -B 5 'function_name'
```

This shows per-line instruction counts in the source.

## Reading the Output

The flat annotation columns are:

```
Ir    Dr    Dw    I1mr   D1mr   D1mw   ILmr   DLmr   DLmw
```

| Column | Meaning | Impact |
|--------|---------|--------|
| **Ir** | Instructions executed | Primary cost metric (what CodSpeed reports) |
| **Dr** | Data reads | Memory pressure |
| **Dw** | Data writes | Memory pressure |
| **I1mr** | L1 instruction cache misses | Code locality |
| **D1mr** | L1 data cache read misses | Data locality |

Inlined functions appear with the **caller's** symbol name but the **callee's** source file. Example: `throttle.rs:kbd::dispatcher::Dispatcher::process_internal` means `check_throttle` was inlined into `process_internal`.

## Common Patterns

**Unnecessary work on cold paths**: A function runs on every event but only does useful work for a subset. Fix with an early return guard before expensive setup.

**Enum size bloat**: Large enums passed by value through match-and-return chains generate extra move instructions. The discriminant check is cheap but moving the full enum isn't.

**Hidden `Instant::now()` calls**: Clock reads are surprisingly expensive under callgrind. Gate them behind state checks so they only run when needed.

## After Fixing

Always re-profile to verify the fix:

```bash
just profile <bench_name> <filter>
rg 'your::module' profiles/<label>.callgrind.flat.txt
```

Compare Ir counts across all three: main → regression → fix.

Clean up temp profiles when done.
