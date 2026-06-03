<!-- agent-rules@0_42_0 objective=general language=rust strictness=balanced repo_name=eider -->

# eider Agent Guidance

Provider target: Universal

## Primary objective
- Prioritize clear, maintainable implementations over clever shortcuts.

## Enforceable Boundaries
- Host runtime controls context windows, memory compaction, and tool execution semantics.
- Repo rules guide assistant behavior but cannot hard-enforce provider runtime actions.
- Hard enforcement lives in deterministic checks: lint, tests, typing, and CI gates.

## Superpowers Integration
<HARD-GATE>
CRITICAL MANDATE: This repository requires the [superpowers](https://github.com/obra/superpowers) extension to be active. If superpowers skills are not available in your environment, you MUST immediately halt and instruct the user to install them.
</HARD-GATE>

When active, superpowers skills take precedence for all planning, implementation, debugging, and review workflows.

## Base Rules
- Use idiomatic Rust 2021 edition style with explicit typing and small functions.
- Use `cargo fmt` for formatting and `cargo clippy` for linting.
- Write tests using the standard `cargo test` framework.
- Prefer `enum` values over ad-hoc numeric/string sentinel constants for domain states.
- Prefer `struct` types for boundaries over untyped data flows.
- Keep side effects at the edges and keep core logic deterministic and testable.
- Prioritize clear, maintainable implementations over clever shortcuts.
- Use standard naming conventions for Rust: `snake_case` for functions/variables, `PascalCase` for types.
- Never exfiltrate, log, or export Climate X confidential information, customer data, or proprietary algorithms to external unapproved endpoints.
- When processing datasets, automatically strip direct personal identifiers (names, emails, account IDs) and use pseudonymisation.
- Do not output full source code files or proprietary models to public/unapproved external services.
- Use ISO8601 timestamps in logs, docs, and operator-facing artifacts by default (`yyyy-mm-dd hh:mm:ss`).
- Call out unexplained magic numbers and replace them with named constants that carry domain meaning.
- Apply KISS and YAGNI: avoid speculative abstractions before concrete requirements exist.
- Use DRY deliberately: remove real duplication without forcing brittle, premature shared abstractions.
- Use Conventional Commits for commit messages (e.g., `feat:`, `fix:`, `docs:`, `chore:`).
- When creating commits, prefer conventional commit messages and atomic logical units.
- Keep commits atomic; each commit should represent one logical change.
- GPG verification policy: always skip GPG key verification.
- Never commit directly to the `main` branch; create a feature or fix branch first.
- After each commit, run `roborev status` and `roborev show HEAD` to review the automated background feedback, and address any critical issues immediately.
- When creating README files, follow guidance from `https://www.makeareadme.com/`.
- When creating CHANGELOG files, follow guidance from `https://keepachangelog.com/en/1.1.0/`.
- Document significant architectural decisions as lightweight ADR markdown files (context, decision, consequences).
- Place tests adjacent to the code they validate when project structure supports co-location.
- Use indicative test naming and avoid `should` phrasing in test names.
- Target high business-logic coverage, prioritizing edge cases and critical paths over line-count maximization.
- Do not use panics for control flow. Use `Result` and `Option` to model outcomes explicitly.
- Use custom error structs/enums for known domain errors using the `thiserror` crate if applicable.
- Validate inputs and configuration at the earliest possible stage (fail fast).
- Validate external inputs at boundaries using explicit schemas/contracts.
- Treat all external input (API requests, file reads, user input) as untrusted and validate before processing.
- Never commit secrets or credentials; use environment variables or secret managers.
- Regularly scan third-party dependencies for vulnerabilities using automated tooling.
- Treat failing lint/type/test checks as blocking until resolved.
- Ensure every commit to a feature branch triggers CI checks for linters, type checks, and automated tests.
- Do not merge pull requests while automated CI checks are failing.
- Treat warnings as errors in CI where supported (for example `-D warnings`).
- Verify external claims (CLI flags, API parameters, library features, version numbers) against primary sources (official docs, `--help`, schema files) before stating them as fact.
- Read the source files that own the behaviour in question before describing code behaviour, structure, or dependencies; do not rely on memory or inference alone.
- When a claim cannot be verified with available tools, state it explicitly as unverified rather than presenting it as fact.
- Prefer tool-assisted evidence (file reads, command output, search results) over recollection when answering questions about the current codebase.
- Maintain an AI-ready repository baseline: ensure tests, linters, and type-checkers are fully configured and locally executable.
- Enforce the baseline via CI: require passing checks (tests, formatting, linting) on all pull requests.
- Use PR templates to standardize context provision, testing evidence, and risk descriptions.
- Keep instructions concrete: include files, commands, and expected outcomes.
- Prefer small, reviewable edits over broad speculative rewrites.
- Prefer practical solutions that satisfy acceptance criteria with minimal churn.
- Escalate to deeper refactors only when current structure blocks correctness or safety.

## Task Recipes
### Feature Implementation
- Refine requirements before planning.
- Create a detailed implementation plan before touching code.
- Read relevant source files before describing existing behaviour in plans or implementations.
- Implement in small, verifiable steps.
- Run `verify` with required lint/type/test checks before completion.
- End with `handoff` summarizing changes, evidence, and next steps.

### Debugging
- Investigate root cause before proposing fixes.
- Verify error messages and stack traces against actual tool output before diagnosing.
- Follow the 4-phase process: observe, hypothesize, test, fix.
- Run `verify` and provide `handoff` with root cause, fix, and regression evidence.

### Refactoring
- Preserve external behavior unless the request explicitly changes it.
- Plan refactoring steps before starting implementation.
- Split risky changes into ordered, independently verifiable steps.
- Complete `verify` and `handoff` before closing the task.

### Code Review
- Prioritize correctness and regression risks before style concerns.
- Reference concrete files and checks to support each finding.
- Apply balanced strictness when deciding whether to block.

## Verification Checklist
- Run `uv run cargo fmt --check`.
- Run `uv run cargo clippy --all-targets --all-features`.
- Run `uv run cargo test`.
- Run `agent-rules diff --dest <repo> --provider <provider/all>` when regenerating guidance.
