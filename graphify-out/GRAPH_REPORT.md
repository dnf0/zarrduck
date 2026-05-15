# Graph Report - duckdb_geozarr  (2026-05-15)

## Corpus Check
- 25 files · ~23,277 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 193 nodes · 175 edges · 22 communities (18 shown, 4 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `54ff9f5c`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- [[_COMMUNITY_Community 0|Community 0]]
- [[_COMMUNITY_Community 1|Community 1]]
- [[_COMMUNITY_Community 2|Community 2]]
- [[_COMMUNITY_Community 3|Community 3]]
- [[_COMMUNITY_Community 4|Community 4]]
- [[_COMMUNITY_Community 5|Community 5]]
- [[_COMMUNITY_Community 6|Community 6]]
- [[_COMMUNITY_Community 7|Community 7]]
- [[_COMMUNITY_Community 8|Community 8]]
- [[_COMMUNITY_Community 9|Community 9]]
- [[_COMMUNITY_Community 10|Community 10]]
- [[_COMMUNITY_Community 11|Community 11]]
- [[_COMMUNITY_Community 12|Community 12]]
- [[_COMMUNITY_Community 13|Community 13]]
- [[_COMMUNITY_Community 14|Community 14]]
- [[_COMMUNITY_Community 15|Community 15]]
- [[_COMMUNITY_Community 16|Community 16]]
- [[_COMMUNITY_Community 17|Community 17]]
- [[_COMMUNITY_Community 18|Community 18]]

## God Nodes (most connected - your core abstractions)
1. `/graphify` - 16 edges
2. `What You Must Do When Invoked` - 15 edges
3. `duckdb_geozarr Agent Guidance` - 7 edges
4. `Ubiquitous Language` - 7 edges
5. `Language` - 6 edges
6. `Part B - Semantic extraction (parallel subagents)` - 6 edges
7. `Task 1: Initialize Rust Project and Extension Entrypoint` - 6 edges
8. `DuckDB GeoZarr Extension Design (`duckdb_geozarr`)` - 6 edges
9. `Task Recipes` - 5 edges
10. `Deepening` - 5 edges

## Surprising Connections (you probably didn't know these)
- None detected - all connections are within the same source files.

## Communities (22 total, 4 thin omitted)

### Community 0 - "Community 0"
Cohesion: 0.06
Nodes (32): code:bash (mkdir -p graphify-out), code:bash ($(cat graphify-out/.graphify_python) -c "), code:bash ($(cat graphify-out/.graphify_python) -c "), code:bash ($(cat graphify-out/.graphify_python) -c "), code:bash ($(cat graphify-out/.graphify_python) -c "), code:bash ($(cat graphify-out/.graphify_python) -c "), code:bash ($(cat graphify-out/.graphify_python) -c "), code:bash (# Detect the correct Python interpreter (handles pipx, venv,) (+24 more)

### Community 1 - "Community 1"
Cohesion: 0.07
Nodes (26): code:block1 (/graphify                                             # full), code:bash (if [ ! -f graphify-out/.graphify_python ]; then), code:bash ($(cat graphify-out/.graphify_python) -c "), code:bash ($(cat graphify-out/.graphify_python) -c "), code:bash ($(cat graphify-out/.graphify_python) -c "), code:bash ($(cat graphify-out/.graphify_python) -m graphify save-result), code:bash ($(cat graphify-out/.graphify_python) -c "), code:bash ($(cat graphify-out/.graphify_python) -c ") (+18 more)

### Community 2 - "Community 2"
Cohesion: 0.12
Nodes (16): code:toml ([package]), code:rust (#[test]), code:rust (use duckdb::ffi;), code:bash (git add src/table_function.rs tests/test_extension.rs), code:rust (use duckdb::{Connection, Result};), code:rust (use duckdb::ffi;), code:rust (use duckdb::{Connection, Result};), code:bash (git add Cargo.toml src/lib.rs tests/test_extension.rs) (+8 more)

### Community 3 - "Community 3"
Cohesion: 0.17
Nodes (11): customizations, vscode, features, ghcr.io/devcontainers/features/docker-in-docker:2, ghcr.io/devcontainers/features/github-cli:1, ghcr.io/devcontainers/features/python:1, version, image (+3 more)

### Community 4 - "Community 4"
Cohesion: 0.17
Nodes (11): Base Rules, Code Review, Debugging, duckdb_geozarr Agent Guidance, Enforceable Boundaries, Feature Implementation, Primary objective, Refactoring (+3 more)

### Community 5 - "Community 5"
Cohesion: 0.17
Nodes (10): code:bash (git add <files>), code:bash (roborev wait --quiet), code:bash (roborev refine), Overview, Roborev Integration, Step 1: Commit your code, Step 2: Wait for the verdict, Step 3: The Autonomous Refine Loop (If failed) (+2 more)

### Community 6 - "Community 6"
Cohesion: 0.18
Nodes (11): code:bash ($(cat graphify-out/.graphify_python) -c "), code:bash ($(cat graphify-out/.graphify_python) -c "), code:bash ($(cat graphify-out/.graphify_python) -c "), code:bash ($(cat graphify-out/.graphify_python) -c "), code:bash ($(cat graphify-out/.graphify_python) -c "), code:block8 ([Agent tool call 1: files 1-15, subagent_type="general-purpo), code:block9 (You are a graphify extraction subagent. Read the files liste), Part A - Structural extraction for code files (+3 more)

### Community 7 - "Community 7"
Cohesion: 0.2
Nodes (8): 1. In-process, 2. Local-substitutable, 3. Remote but owned (Ports & Adapters), 4. True external (Mock), Deepening, Dependency categories, Seam discipline, Testing strategy: replace, don't layer

### Community 8 - "Community 8"
Cohesion: 0.22
Nodes (7): code:md (# Ubiquitous Language), Example dialogue, Output Format, Process, Re-running, Rules, Ubiquitous Language

### Community 9 - "Community 9"
Cohesion: 0.25
Nodes (6): 1. Explore, 2. Present candidates, 3. Grilling loop, Glossary, Improve Codebase Architecture, Process

### Community 10 - "Community 10"
Cohesion: 0.29
Nodes (5): 1. Frame the problem space, 2. Spawn sub-agents, 3. Present and compare, Interface Design, Process

### Community 11 - "Community 11"
Cohesion: 0.29
Nodes (5): Language, Principles, Rejected framings, Relationships, Terms

### Community 12 - "Community 12"
Cohesion: 0.29
Nodes (6): 1. Purpose & Context, 2. Architecture & Stack, 3. Data Flow & Execution, 4. LLM Integration Strategy, 5. Development & Testing, DuckDB GeoZarr Extension Design (`duckdb_geozarr`)

### Community 13 - "Community 13"
Cohesion: 0.4
Nodes (5): code:bash ($(cat graphify-out/.graphify_python) -c "), code:bash ($(cat graphify-out/.graphify_python) -c "), code:bash ($(cat graphify-out/.graphify_python) -c "), code:bash ($(cat graphify-out/.graphify_python) -c "), For --update (incremental re-extraction)

### Community 14 - "Community 14"
Cohesion: 0.5
Nodes (4): code:bash ($(cat graphify-out/.graphify_python) -c "), code:bash ($(cat graphify-out/.graphify_python) -c "), code:bash ($(cat graphify-out/.graphify_python) -m graphify save-result), For /graphify explain

## Knowledge Gaps
- **112 isolated node(s):** `name`, `image`, `version`, `ghcr.io/devcontainers/features/github-cli:1`, `ghcr.io/devcontainers/features/docker-in-docker:2` (+107 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **4 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `What You Must Do When Invoked` connect `Community 0` to `Community 1`, `Community 6`?**
  _High betweenness centrality (0.127) - this node is a cross-community bridge._
- **Why does `/graphify` connect `Community 1` to `Community 0`, `Community 13`, `Community 14`?**
  _High betweenness centrality (0.117) - this node is a cross-community bridge._
- **Why does `Step 3 - Extract entities and relationships` connect `Community 6` to `Community 0`?**
  _High betweenness centrality (0.039) - this node is a cross-community bridge._
- **What connects `name`, `image`, `version` to the rest of the system?**
  _112 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Community 0` be split into smaller, more focused modules?**
  _Cohesion score 0.06 - nodes in this community are weakly interconnected._
- **Should `Community 1` be split into smaller, more focused modules?**
  _Cohesion score 0.07 - nodes in this community are weakly interconnected._
- **Should `Community 2` be split into smaller, more focused modules?**
  _Cohesion score 0.12 - nodes in this community are weakly interconnected._
