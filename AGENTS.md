# Repository Guidelines

## Project Structure & Module Organization

This is a Rust library crate for an in-memory SQL-like query engine and data store. The crate root is `src/lib.rs`; implementation is split by domain:

- `src/database/`: collections, IDs, schema metadata, JSON storage, load/write helpers.
- `src/parser/`: SQL parsing, AST types, literal parsers, analyzer, and resolvers.
- `src/planner/`: logical plan construction and aggregate calls.
- `src/executor/`: query execution, row handling, evaluation, and executor tests.
- `examples/full_demo/`: independent executable documentation project covering the public API, fixtures, schema loading, references, and SQL query examples.
- `images/`: README and branding assets; these are excluded from published crate packages.

Tests live beside code under `#[cfg(test)]` modules. Shared executor fixtures are in `src/executor/_tests.rs`.

## Build, Test, and Development Commands

- `cargo build`: compile the crate and dependencies.
- `cargo test`: run all unit and module tests.
- `cargo test parser::`: run parser-focused tests by module path.
- `cargo fmt`: format Rust source using `rustfmt`.
- `cargo clippy --all-targets --all-features`: run lint checks across library and tests.
- `cargo doc --no-deps`: generate local API documentation.
- `cargo run --manifest-path examples/full_demo/Cargo.toml`: run the full executable documentation demo; keep this working when public APIs, fixtures, schema behavior, references, or query behavior change.

Run `cargo fmt`, `cargo clippy --all-targets --all-features`, `cargo test`, and `cargo run --manifest-path examples/full_demo/Cargo.toml` before submitting changes.

## Coding Style & Naming Conventions

Use Rust 2024 edition conventions and standard `rustfmt` formatting. Keep modules small and domain-specific. Use `snake_case` for functions, methods, modules, and test names; use `PascalCase` for structs, enums, and traits. Prefer existing module error types over broad string errors. Reuse parser, analyzer, planner, and executor helpers before adding abstractions.

## Testing Guidelines

Add focused tests next to the code they cover using `#[cfg(test)] mod tests`. Name tests by behavior, for example `parses_limit_with_offset` or `executes_left_join_with_missing_rows`. For query behavior crossing parsing, planning, and execution, add coverage in `src/executor/_tests.rs` or the relevant executor module. Include regression tests for bug fixes before changing behavior. Treat `examples/full_demo` as executable documentation: update and run it when public API return types, JSON fixtures, schema loading, references, or query behavior change.

## Commit & Pull Request Guidelines

Recent history uses short Conventional Commit-style subjects such as `fix: ...` and `chore: ...`. Keep subjects imperative and concise. Pull requests should include a brief summary, commands run, linked issues when applicable, and screenshots only when README assets or visual docs change. Note behavior changes to query parsing, ID generation, or JSON output explicitly.

## Security & Configuration Tips

Do not commit generated build output from `target/` or local OS files. Avoid new dependencies unless necessary for parser, database, or executor functionality and justified in the PR. Treat JSON file loading paths and persisted output behavior as user-facing API surface.
