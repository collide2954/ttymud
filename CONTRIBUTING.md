# How to contribute

Thank you for your help with TTYmud. This page tells you how to work on the project.

## Rules

- Write a test first. Make sure that the test fails. Then write the code that makes the test pass.
- Make small commits. Each commit does one thing. All tests pass after each commit.
- Write each commit message on one line, in the imperative. Example: `Add trace to the status line`.
- Keep comments in code to a minimum. Use clear names.
- Add a dependency only when there is a strong reason. Add the reason to the dependency table in [architecture.md](docs/design/architecture.md) in the same commit.
- Write all documents in ASD-STE100 Simplified Technical English. Read [style.md](docs/style.md).

## Before each push

Run these commands. Make sure that each command finishes without errors.

    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

## License

When you send a contribution, you agree that it is under the [Apache License 2.0](LICENSE).
