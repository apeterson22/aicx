# Legacy Python Reference Guidelines

## Scope
This guide applies only to the legacy Python prototype under `aicx/`.

## Project Structure
- Keep the Python prototype as a compatibility reference only.
- Prefer `aicx_native` when it is installed; legacy code is a fallback path.
- Update these files only when it helps preserve compatibility or explain Rust behavior.

## Commands
- Existing pytest scaffolding is legacy-only and should not become the primary verification path.
- Treat `cargo test --workspace` and the Python binding build as the primary verification paths.

## Style
- Make minimal edits and avoid expanding the prototype with new architecture.
- Keep wrappers small, explicit, and easy to delete once the native path fully replaces them.

## Security
- Do not treat the Python prototype as the production archive engine.
- Prefer the Rust core for any new security-sensitive behavior and validation.
