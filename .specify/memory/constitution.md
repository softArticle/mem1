<!--
Sync Impact Report
- Version change: N/A (template) -> 1.0.0
- Modified principles:
  - Template Principle 1 -> I. Rust-First Core
  - Template Principle 2 -> II. Local-First and Privacy by Default
  - Template Principle 3 -> III. Python SDK Parity Contract
  - Template Principle 4 -> IV. Test and Quality Gates (NON-NEGOTIABLE)
  - Template Principle 5 -> V. Deterministic Observability and Compatibility
- Added sections:
  - Technical Boundaries
  - Delivery Workflow and Quality Gates
- Removed sections:
  - None
- Templates requiring updates:
  - ✅ `.specify/templates/plan-template.md` (verified compatible; constitution check placeholder is generic and sufficient)
  - ✅ `.specify/templates/spec-template.md` (verified compatible; supports mandatory scenarios, requirements, and measurable outcomes)
  - ✅ `.specify/templates/tasks-template.md` (verified compatible; supports test, integration, and polish phases required by this constitution)
  - ⚠ pending `.specify/templates/commands/*.md` (path missing in repository; cannot validate command-template references)
- Runtime guidance updates:
  - ✅ `README.md` not present; no update required
  - ✅ `docs/quickstart.md` not present; no update required
- Follow-up TODOs:
  - None
-->
# Memory Service Constitution

## Core Principles

### I. Rust-First Core

All production runtime components of the memory service MUST be implemented in Rust.
Core capabilities (storage engine, retrieval, embedding orchestration, indexing, and API
runtime) MUST NOT require non-Rust service dependencies. Python is allowed only as an SDK
consumer boundary, not as a backend runtime dependency.
Rationale: this keeps performance, reliability, and deployability consistent with the
"pure Rust" objective.

### II. Local-First and Privacy by Default

The system MUST run fully on a local machine without mandatory cloud connectivity. Memory
data MUST remain local by default, and any external model/provider integration MUST be
explicitly opt-in. Sensitive data MUST be encrypted at rest and redacted in logs.
Rationale: localized AI memory services are only trustworthy when privacy is the default
and offline operation is first-class.

### III. Python SDK Parity Contract

A Python SDK MUST be provided and maintained as a stable client contract for the Rust
service. Every public Rust API capability exposed for external use MUST have equivalent
Python SDK coverage or an explicitly documented exception. SDK versioning MUST track API
compatibility and publish migration notes for breaking changes.
Rationale: Python is the primary integration surface for many AI workflows and must not
lag behind the Rust service.

### IV. Test and Quality Gates (NON-NEGOTIABLE)

All features MUST include unit tests in Rust and integration tests that validate end-to-end
memory flows. Any API change MUST include contract tests for both Rust and Python SDK
bindings. CI MUST fail on test failures, lint failures, or formatting violations.
Rationale: cross-language correctness and local data integrity require strict quality gates.

### V. Deterministic Observability and Compatibility

The service MUST emit structured logs and stable error codes for every externally visible
failure mode. Retrieval and write operations MUST include deterministic trace identifiers for
reproducibility. Backward compatibility policy MUST be explicit: breaking API/storage changes
require major version increments and documented migration paths.
Rationale: observability and compatibility discipline are required for operating stateful
memory systems safely over time.

## Technical Boundaries

- Primary language MUST be Rust (stable toolchain pinned in repository metadata).
- Allowed SDK language for first-party support is Python.
- Service interfaces SHOULD prefer transport-neutral contracts (e.g., HTTP/gRPC + schema).
- Local storage format changes MUST include migration tooling and rollback guidance.
- Default deployment target is a single local host process; distributed mode is optional and
  MUST NOT weaken local-only guarantees.

## Delivery Workflow and Quality Gates

- Every feature specification MUST include user scenarios, acceptance criteria, and measurable
  outcomes for latency, correctness, or reliability.
- Implementation plans MUST pass constitution checks before research/design execution.
- Tasks MUST be organized by independently testable user stories and include explicit testing
  work for Rust core and Python SDK.
- Pull requests MUST include evidence of tests run (`cargo test`, lint/format checks, and SDK
  integration checks where relevant).
- Releases MUST include a compatibility note covering API and storage implications.

## Governance

This constitution overrides conflicting local practices and templates. Amendments MUST be
proposed in writing, include rationale and migration impact, and be approved before merge.

Versioning policy for this constitution follows semantic versioning:

- MAJOR: Removes or redefines a principle/governance rule in a backward-incompatible way.
- MINOR: Adds a new principle/section or materially expands mandatory guidance.
- PATCH: Clarifies wording without changing required behavior.

Compliance review is mandatory for every plan and pull request. Reviewers MUST verify that
constitution checks are explicit and passed, or that any temporary exception is documented
with owner, scope, and expiration date.

**Version**: 1.0.0 | **Ratified**: 2026-03-06 | **Last Amended**: 2026-03-06
