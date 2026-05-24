# Legacy Python Reference Guidelines

## Scope
This guide applies only to the compatibility shim under `aicx/`.

## Project Structure
- Keep only the shim modules needed for imports like `import aicx` and `import aicx.container`.
- Prefer `aicx_native` for all archive operations, including `extract`.
- Update these files only when it helps preserve compatibility or explain Rust behavior.

## Commands
- Treat `cargo test --workspace` and the Python binding build as the primary verification paths.

## Style
- Make minimal edits and avoid reintroducing archive logic into the shim.
- Keep wrappers small, explicit, and easy to replace.

## Security
- Do not treat the shim as the production archive engine.
- Prefer the Rust core for any new security-sensitive behavior and validation.
- Do not add new compatibility modules; this package should remain a thin forwarder only.
