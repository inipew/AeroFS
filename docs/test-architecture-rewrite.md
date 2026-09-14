# AeroFS Test Architecture Rewrite

## Goal

The rewrite treats tests as executable product contracts rather than historical records of implementation phases. A refactor should require changing a test only when externally meaningful behavior, an intentional architectural boundary, or a supported contract changes.

The migration is deliberately incremental. Existing tests remain as an oracle until their behavior has an equivalent test in the new architecture.

## Phase 1 baseline audit

At the start of this work, `backend/tests` contains **49 top-level Rust integration-test crates**. **35 of 49** are named after historical implementation milestones or priority labels (`h*`, `hardening*`, `p*`, `phase*`, `plan*`, or `performance_phase*`) rather than the product behavior they protect.

The naming is not itself a correctness bug, but inspection found several recurring maintenance hazards:

1. **Composition-root duplication.** API, transfer, lifecycle, and other suites repeatedly create temp directories, construct SQLite URLs, run initialization, build the application, and create routers independently.
2. **Database-fixture duplication.** Several tests create ad-hoc SQLite pools and invoke `sqlx::migrate!` directly. Some use `sqlite::memory:`, which is a risky default with a connection pool because an in-memory database is connection-scoped.
3. **Timing assumptions.** Async lifecycle tests frequently use fixed sleeps or hand-written fixed-count polling loops. Those encode scheduler speed rather than a behavior deadline.
4. **Mixed abstraction levels.** A single historical phase file can contain domain/state tests, persistence tests, runtime tests, and integration tests, making ownership and failure diagnosis unclear.
5. **Implementation-text assertions.** For example, `h8_upload_session_stability_tests.rs` reads Rust source files and asserts exact source substrings and function-body structure. That can fail after a semantics-preserving refactor and should eventually be replaced by a behavioral contract or, where truly architectural, a compile-time boundary test.

Phase 1 does **not** delete those tests. It establishes the replacement infrastructure first.

## Shared TestKit introduced in Phase 1

`backend/tests/support` is the common integration-test foundation:

- `TestDatabase::migrated` creates an isolated file-backed SQLite database and runs the real migration set without seed data.
- `TestDatabase::seeded` mirrors application database initialization, including defaults.
- `TestAppBuilder` owns a temporary storage root, seeded database, application state, runtime owner, and router; it can seed files and select the initial runtime phase.
- `TestApp::login` / `login_admin` centralize authenticated HTTP setup.
- `eventually` probes an observable condition until a named deadline rather than sleeping for an assumed completion time.

The TestKit is intentionally small in Phase 1. Provider doubles, event recorders, deterministic failure injection, and richer domain builders should be added only when the subsystem migration demonstrates a real need.

## Phase 2 transfer migration

Phase 2 adds a transfer-specific TestKit layer without exposing `TransferManager` through the application boundary:

- `support::transfer` owns actor creation, application-level transfer submission/listing, job lookup, and condition-based status convergence.
- `transfer_lifecycle_tests.rs` owns product behavior: copy/move, recursive copy, durable terminal history, retry from failed durable state, concurrent retry admission, dismiss/clear, ownership, cancellation cleanup, and connection-drain behavior.
- `transfer_engine_invariants_tests.rs` owns intentionally implementation-level guarantees that cannot be expressed faithfully at the application boundary: retry permission revalidation, unavailable/disabled providers, cleanup-phase move recovery, retry CAS, finalizing cancellation, progress checkpoint persistence, enum/string stability, and terminal DB fallback parity.

The replacement suites were first run beside all legacy transfer suites. After that equivalence run passed, `transfer_tests.rs`, `plan3_transfer_lifecycle_invariants_tests.rs`, `plan38_transfer_lifecycle_tests.rs`, and `plan39_transfer_hardening_tests.rs` were removed. The migration therefore reduces the top-level Rust integration-test crate count from **49 to 47** and the historically named subset from **35 to 32**.

`p1_transfer_boundary_tests.rs` remains intentionally: it protects architecture boundaries rather than transfer lifecycle behavior and belongs to a later contract/architecture migration.

One legacy transfer test was intentionally not migrated as a requirement: `test_transfer_dynamic_limits_update` only changed a settings value and then asserted that an unrelated small copy still completed. It did not verify concurrency changed. Runtime/settings concurrency behavior belongs in the settings/runtime migration with an observable concurrency contract rather than preserving that weak assertion.

## Phase 3 sync and recovery migration

Phase 3 keeps sync tests behind one top-level `sync_tests.rs` integration crate and uses modules for lifecycle, history, contracts, and recovery. This avoids increasing Rust integration-crate fragmentation while giving failures clear behavioral ownership.

- `support::sync` owns application-level job creation, bounded history queries, and condition-based sync convergence.
- `sync/lifecycle.rs` verifies source-wins execution, durable terminal history, conflicts, and conflict resolution through the application boundary.
- `sync/history.rs` verifies real keyset pagination through `SyncService`, including duplicate-free job traversal and strict operation scoping per job.
- `sync/contracts.rs` replaces source-text architecture assertions with trait-backed `SyncService` tests and preserves pure manifest-diff/conflict-filename contracts.
- `sync/recovery.rs` uses file-backed real migrations and durable state injection to verify multi-page interrupted recovery, transfer-completion idempotency, restart convergence, and bounded streaming-plan batches.

The recovery test deliberately uses **300 operations**, crossing the 256-row recovery batch boundary, and the streaming planner test uses **300 files**, proving that operation batches remain bounded by `SCAN_BATCH_SIZE` without asserting source-code shape.

Replacement and legacy sync coverage first passed together in CI #436. Only after that equivalence proof were `p2_sync_boundary_tests.rs` and the three sync-specific cases in mixed `plan59_lifecycle4_architectural_upgrade_tests.rs` removed. Non-sync `plan59` coverage remains for later subsystem phases. `plan40_sync_architecture_tests.rs` also remains because its actual contents belong to request middleware, event replay, staging visibility, and transfer behavior rather than sync.

The top-level Rust integration-test count remains **47** because one historical sync crate is replaced by one grouped sync crate; the historically named subset drops from **32 to 31**.

## Rewrite rules

Every legacy test is handled with the following sequence:

```text
identify the requirement protected by the legacy test
        ↓
classify it as product behavior / contract / architectural invariant / obsolete
        ↓
write the equivalent test at the lowest useful boundary
        ↓
run legacy + replacement tests together
        ↓
remove the legacy test only after equivalence is demonstrated
```

A legacy assertion is not automatically a requirement. If a test protects an implementation detail that is no longer intentional, the correct migration may be deletion after the real behavior is covered.

New tests should follow these rules:

- Name tests after behavior and outcome, not project phase numbers.
- Prefer domain/application tests over HTTP tests when HTTP is not part of the behavior under test.
- Use the real migration set for persistence contracts.
- Avoid `sqlite::memory:` for pooled integration fixtures unless a test intentionally owns exactly one SQLite connection.
- Never use an arbitrary sleep as proof that asynchronous work completed. Observe a state/event/condition with a deadline instead.
- Prefer deterministic barriers, notifications, controlled providers, or `eventually` over scheduler timing.
- Assert public/application contracts rather than private collection layout, task count ordering, or exact source text unless that structure is itself an explicit architectural rule.
- A regression test belongs at the lowest layer that can reproduce the bug faithfully.

## Target suite shape

Rust treats each top-level file under `tests/` as a separate integration crate, so migration should also reduce unnecessary top-level crate fragmentation. The intended logical ownership is:

```text
backend/tests/
├── support/          # shared TestKit; not a standalone test crate
├── application/      # modules grouped under a small number of top-level targets
├── api/              # HTTP contract behavior
├── contracts/        # provider/filesystem/persistence contracts
└── resilience/       # cancellation, restart, recovery, concurrency
```

The exact physical grouping can evolve during migration; behavior ownership matters more than preserving this sketch literally.

## Migration phases

### Phase 1 — Foundation and audit

- establish shared TestKit
- establish condition-based waiting
- document the legacy-suite risks and migration rules
- keep every legacy test running

### Phase 2 — Transfer lifecycle

- consolidate queue/execution/cancellation/retry/persistence/ownership behavior around `support::transfer`
- separate product behavior from engine invariants
- replace fixed transfer polling loops with named convergence deadlines
- validate replacement and legacy transfer suites together
- remove superseded `plan3`, `plan38`, `plan39`, and duplicate `transfer_tests` ownership

### Phase 3 — Sync and recovery

- consolidate sync lifecycle/history/contracts/recovery behind one grouped integration target
- replace source-text sync boundary assertions with trait-backed contracts
- verify >1 recovery page and >1 streaming batch behavior with durable/file-backed fixtures
- validate legacy and replacement sync coverage together
- remove `p2_sync_boundary_tests.rs` and sync ownership from mixed `plan59`

### Phase 4 — File/VFS operations

Consolidate local VFS, mutation concurrency, upload, archive transformation, filesystem boundary, and provider contract tests using reusable filesystem/provider fixtures.

### Phase 5 — Connection/provider lifecycle

Cover connection persistence, provider generation, lease/reclamation, reconnect/error behavior, and credential boundaries with controlled test doubles rather than source-shape assertions.

### Phase 6 — API/authz contracts

Separate HTTP serialization/status contracts from application authorization behavior; centralize authenticated request fixtures and user/permission builders.

### Phase 7 — Remaining subsystems and runtime

Migrate settings, search, archive, trash, events/replay, runtime/bootstrap/shutdown, CLI/config, and security suites. Delete historical phase/hardening files after their supported behavior is represented in the new suites.

## Exit criteria

The rewrite is complete when:

- test names communicate requirements without knowledge of AeroFS refactor history;
- shared setup is owned by TestKit instead of copied across suites;
- asynchronous tests wait on observable conditions rather than arbitrary delays;
- source-text tests are eliminated except for intentionally documented architecture checks with no stronger compile-time alternative;
- top-level integration crate count is materially reduced;
- subsystem refactors that preserve behavior no longer require broad test repairs;
- CI failures identify a behavioral area rather than a historical implementation phase.
