# Contributing

Thanks for your interest in cmdstr!

## Getting Started

1. Fork the repo on GitHub.
2. Clone your fork:
   ```bash
   git clone https://github.com/ceeejeey/cmdstr.git
   ```
3. Build:
   ```bash
   cargo build
   ```

## Development

- The crate lives under `cmd-store/`.
- Run `cargo build` from the workspace root.
- Run `cargo test` to run tests.

### Coding Style

- Keep it simple. No unnecessary dependencies.
- Follow existing patterns in the codebase.
- No commented-out code.

## Pull Requests

1. Create a feature branch from `main`.
2. Make your changes.
3. Ensure it builds cleanly with no warnings.
4. Open a PR against `main`.
5. Keep PRs focused on a single concern.

## Packaging

For PPA releases, bump the version in `cmd-store/Cargo.toml` and update `cmd-store/debian/changelog`:

```bash
dch -i
debuild -S -sa
```
