<!-- agent-rules@1_5_1 objective=general language=python,rust strictness=balanced repo_name=eider -->

# eider Gemini Guidance

Provider target: Gemini

This file supplements `AGENTS.md` with provider-specific configuration.

<HARD-GATE>
You MUST read the full engineering guidance in the file `AGENTS.md` before responding to ANY user request.
</HARD-GATE>

- You should not use web search or sources of information outside of this repo.
- Always adhere to the architectural principles defined in `.cursorrules`.

<SUPERPOWERS-DEVELOPMENT-LOOP>
You lack the native superpowers plugin architecture, so you MUST manually enforce its state machine.

For ANY feature work, bug fix, or codebase change, you must locate your current state in this loop and execute the required prerequisite skill BEFORE taking any other action:

1. **State: Idea / Request Received**
   - Required Skill: `brainstorming`
   - Output: Validated design spec. (Do NOT write code or plans yet).

2. **State: Spec Exists, Needs Plan**
   - Required Skill: `writing-plans`
   - Output: Step-by-step implementation plan. (Do NOT write implementation code yet).

3. **State: Ready to Execute Plan**
   - Required Skill: `using-git-worktrees`
   - Output: An isolated workspace setup. (Do NOT execute tasks in `main`).

4. **State: In Worktree, Ready to Write Code**
   - Required Skills: `subagent-driven-development` (or `executing-plans`), AND `test-driven-development`
   - Output: Tests written first, then minimal passing implementation code.

5. **State: Code is Written, Claiming "Done"**
   - Required Skill: `verification-before-completion`
   - Output: Hard proof that tests/linters pass before you are allowed to say "I'm finished."

6. **State: Verified & Ready to Merge**
   - Required Skill: `finishing-a-development-branch` (and `roborev-integration` if applicable)
   - Output: Code is reviewed, committed, and integrated.

**YOUR ENFORCEMENT PROTOCOL:**
Before answering the user or taking any action, you must:
1. Output a `<state-evaluation>` block where you identify which of the 6 states you are currently in.
2. If you have not completed the required skill for the *previous* states, you MUST stop and execute them first (e.g., if you are asked to write a plan but have no spec, you must execute `brainstorming`).
3. You must use `view_file` to read the required skill for your current state and announce that you are following it.
</SUPERPOWERS-DEVELOPMENT-LOOP>
