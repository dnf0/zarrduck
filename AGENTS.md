<!-- agent-rules@1_7_1 objective=general language=rust strictness=balanced repo_name=eider -->

# eider Agent Guidance

Provider target: Universal

## Division of Responsibilities
- **AGENTS.md (This file):** Defines the "What" (Core architectural boundaries, coding standards, language rules, and repository requirements).
- **Provider Files (e.g. GEMINI.md, CLAUDE.md):** Defines the "How" (Operational workflows, tool usage loops, and provider-specific state machines).
- **Conflict Resolution:** If there is a conflict, the Provider File takes precedence for behavioral execution, while AGENTS.md takes precedence for coding standards.

## Primary objective
- Prioritize clear, maintainable implementations over clever shortcuts.

## Enforceable Boundaries
- Host runtime controls context windows, memory compaction, and tool execution semantics.
- Repo rules guide assistant behavior but cannot hard-enforce provider runtime actions.
- Hard enforcement lives in deterministic checks: lint, tests, typing, and CI gates.

## Superpowers Integration
<HARD-GATE>
This repository vendors the [superpowers](https://github.com/obra/superpowers) skills directly into `.agents/skills/` (delivered by agent-rules — see `.agents/skills/SUPERPOWERS_VERSION` for the pinned commit). Runtimes that natively load `.agents/skills/` (for example agy / antigravity) pick them up automatically. If your runtime does not auto-load that directory, point it at `.agents/skills/` — do NOT git-clone the superpowers repository or fetch the skills yourself.
</HARD-GATE>

superpowers skills take precedence for all planning, implementation, debugging, and review workflows.

## Base Rules
- Use Rust with explicit ownership boundaries and narrow, composable modules.
- Avoid `unwrap`/`expect` in production paths; propagate typed errors with context.
- Prefer `Result`-centric APIs and domain enums/newtypes over primitive flags.
- Isolate `unsafe` blocks and document invariants at each unsafe boundary.
- Use `cargo fmt` to keep formatting consistent.
- Use `cargo clippy` to catch common mistakes and improve code quality.
- Manage dependencies and feature flags explicitly in `Cargo.toml`.
- Use `cargo test` for automated testing.
- Prefer immutable data flow and explicit lifetimes/borrowing at API boundaries.
- Prioritize clear, maintainable implementations over clever shortcuts.
- Version APIs explicitly, preferably in URL paths (for example `/api/v1/...`).
- Model resources with nouns and rely on HTTP verbs for operations.
- Use standard HTTP status codes consistently (for example 200, 201, 400, 404).
- Use consistent JSON payloads with camelCase keys (for example `{ "data": [...] }` and `{ "error": { "message": "..." } }`).
- Favor Third Normal Form (3NF) for relational schema design unless denormalization is justified.
- Use snake_case names for database objects; plural table names; primary key column `id`; foreign keys `{referenced_table_singular}_id`.
- Manage schema changes through migrations; avoid manual production schema edits.
- Prefer backward-compatible, non-destructive production migrations and phase removals over multiple releases.
- Prefer type-safe query builders for database interactions; use raw SQL only when necessary.
- Avoid N+1 query patterns by using eager loading strategies where appropriate.
- Wrap multi-statement write operations in atomic transactions.
- Index all foreign keys and add indexes for frequently filtered, joined, or ordered columns.
- Never exfiltrate, log, or export Climate X confidential information, customer data, or proprietary algorithms to external unapproved endpoints.
- When processing datasets, automatically strip direct personal identifiers (names, emails, account IDs) and use pseudonymisation.
- Do not output full source code files or proprietary models to public/unapproved external services.
- If `graphify-out/GRAPH_REPORT.md` exists in the repository, you MUST read it before answering architectural questions or searching raw files.
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
- Run `uvx --from graphifyy graphify update .` to rebuild the knowledge graph before pushing your branch to a remote.
- When creating README files, follow guidance from `https://www.makeareadme.com/`.
- When creating CHANGELOG files, follow guidance from `https://keepachangelog.com/en/1.1.0/`.
- Document significant architectural decisions as lightweight ADR markdown files (context, decision, consequences).
- Place tests adjacent to the code they validate when project structure supports co-location.
- Use indicative test naming and avoid `should` phrasing in test names.
- Target high business-logic coverage, prioritizing edge cases and critical paths over line-count maximization.
- Do not use errors for control flow when return types can model outcomes explicitly.
- Use custom error classes for known domain errors.
- Prefer structured logging; avoid `console.log` in production code and use a dedicated logging library when available.
- Validate inputs and configuration at the earliest possible stage (fail fast).
- Validate external inputs at boundaries using explicit schemas/contracts.
- Treat all external input (API requests, file reads, user input) as untrusted and validate with schemas before processing.
- Never commit secrets or credentials; use environment variables or secret managers.
- Regularly scan third-party dependencies for vulnerabilities using automated tooling.
- Treat failing lint/type/test checks as blocking until resolved.
- Ensure every commit to a feature branch triggers CI checks for linters, type checks, and automated tests.
- Do not merge pull requests while automated CI checks are failing.
- Treat warnings as errors in CI where supported (for example `-Werror`).
- Verify external claims (CLI flags, API parameters, library features, version numbers) against primary sources (official docs, `--help`, schema files) before stating them as fact.
- Read the source files that own the behaviour in question before describing code behaviour, structure, or dependencies; do not rely on memory or inference alone.
- When a claim cannot be verified with available tools, state it explicitly as unverified rather than presenting it as fact.
- Prefer tool-assisted evidence (file reads, command output, search results) over recollection when answering questions about the current codebase.
- Maintain an AI-ready repository baseline: ensure tests, linters, and type-checkers are fully configured and locally executable.
- Enforce the baseline via CI: require passing checks (tests, formatting, linting) on all pull requests.
- Embed observability hooks (structured logging, metrics, traces) into all production code paths.
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
- Run `cargo fmt -- --check`.
- Run `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- Run `cargo test --workspace --all-features`.
- Run `agent-rules diff --dest <repo> --provider <provider/all>` when regenerating guidance.
