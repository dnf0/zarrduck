---
name: roborev-integration
description: Guides the agent to strictly integrate with Roborev's continuous code review loop. Must be used immediately after any code is implemented and committed, before moving to the next task.
---

# Roborev Integration

## Overview

This repository uses [Roborev](https://www.roborev.io/) for continuous background code review. Roborev is configured to run automatically via a `post-commit` Git hook.

To ensure the tightest possible feedback loop and prevent overwhelming the auto-fixer, you must strictly follow this micro-commit and micro-review process.

<HARD-GATE>
You are NOT allowed to proceed to the next step in your implementation plan until the code you just wrote has been committed, reviewed by Roborev, and any resulting findings have been completely fixed.
</HARD-GATE>

## The Process

Whenever you complete a tiny implementation step (e.g., making a single test pass):

### Step 1: Commit your code
Commit your changes immediately. This will trigger the Roborev background review hook.
```bash
git add <files>
git commit -m "fix: make test pass"
```

### Step 2: Wait for the verdict
Immediately after committing, you must check the status of the background review by running:
```bash
roborev wait --quiet
```
This command blocks until the review finishes.
- If it exits with `0` (Success): Roborev approved your code. You may proceed to the next task in your plan.
- If it exits with `1` (Failure): Roborev found issues. **You must STOP and proceed to Step 3.**

### Step 3: The Autonomous Refine Loop (If failed)
If Roborev rejected your commit, you must trigger its autonomous refine loop to fix the issues before you are allowed to write any new features.

Run:
```bash
roborev refine
```
*Note: `roborev refine` is a fully autonomous iterative loop. It will read the feedback, spin up an agent to fix the code, run the tests, commit the fixes, and re-review itself until the code passes.*

Do not interrupt or attempt to manually fix the code while `roborev refine` is running.

### Step 4: Verification
Once `roborev refine` finishes successfully, the code is clean. You may now proceed to the next step in your Superpowers implementation plan.
