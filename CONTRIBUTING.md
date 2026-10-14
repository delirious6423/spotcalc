# Contributing to spotcalc

Thanks for helping! Bug reports, ideas and pull requests are all welcome.

## Reporting bugs

Please include your distro, desktop (GNOME, KDE, Sway, ...), Wayland or X11, your GPU, and what
you typed. Logs help: run `RUST_LOG=spotcalc=debug spotcalc` from a terminal.

If a calculation gives a wrong or surprising answer, check it in
[fend](https://printfn.github.io/fend/) first: spotcalc uses fend for all math, so those bugs
belong upstream.

## Development

```bash
cargo run                     # open the window from a debug build
RUST_LOG=spotcalc=trace cargo run   # also log every frame (useful for redraw issues)
```

Before opening a pull request, make sure all of these pass without warnings:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test                    # or: cargo nextest run
cargo deny check              # licences and advisories (https://github.com/EmbarkStudios/cargo-deny)
```

One test downloads real exchange rates and is skipped by default; run it with
`cargo test -- --ignored`.

### Guidelines

- Keep the window fast to open and idle when nothing changes: no continuous redraws, no network
  or disk work on the UI thread.
- New dependencies need a licence that `deny.toml` allows; please explain why in the PR.
- Bug fixes come with a test that failed before the fix.
- If you change behaviour shown in the README, update the README (its examples are tested in
  `tests/readme_examples.rs`).

## Releasing (maintainers)

1. Update `version` in `Cargo.toml`, run `cargo check` so `Cargo.lock` follows, and turn the
   `Unreleased` heading in `CHANGELOG.md` into `## [x.y.z] - YYYY-MM-DD`.
2. Commit, then tag and push: `git tag vx.y.z && git push origin main vx.y.z`.
3. The Release workflow builds the tarball on Ubuntu 22.04 (for glibc 2.35 compatibility),
   bundles all licence texts, and creates a **draft** release with notes from the changelog.
4. Check the draft on GitHub and publish it.

To build the same tarball locally (it will need your machine's newer glibc), install
[cargo-about](https://github.com/EmbarkStudios/cargo-about) and run `scripts/package.sh`;
output goes to `dist/`.

## License

By contributing, you agree that your contributions are licensed under the
[Apache License 2.0](LICENSE), as stated in section 5 of the licence.
