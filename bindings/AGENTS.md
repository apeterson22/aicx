# Python Binding Guidelines

## Scope
This guide applies to everything under `bindings/`.

## Project Structure
- `bindings/python/` owns the maturin project and the PyO3 module.
- Keep the Python wrapper thin and delegate all archive logic to `aicx-core`.

## Commands
- Build the extension with `cd bindings/python && maturin develop`.
- Run binding tests from the same directory after installation.

## Style
- Expose stable Python names that mirror the Rust core operations.
- Prefer small, explicit wrappers over dynamic behavior.

## Security
- Do not bypass Rust-side validation from Python.
- Treat returned metadata as informational; do not execute archive contents.

