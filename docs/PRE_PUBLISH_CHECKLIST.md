# Pre-publish checklist

Use this before pushing to a public repo and before major releases.

## Legal and repo metadata

- [ ] **LICENSE** — Present and correct. Update copyright year/name in `LICENSE` if needed.
- [ ] **README** — Clear title, build/run instructions, link to [docs/](README.md). License section present.

## Secrets and hygiene

- [ ] **No secrets** — No API keys, tokens, or credentials in code or config. Check `.env` is gitignored.
- [ ] **.gitignore** — Covers `target/`, `software/build`, `software/bin`, `software/linux` build artifacts, `__pycache__`, `.env`, `*.so`.
- [ ] **Debug output** — Review `eprintln!`, `dbg!`, and `print()`; remove or gate behind a verbose flag if inappropriate for release.

## Build and test

- [ ] **Build** — `cargo build --release` from repo root.
- [ ] **Rust tests** — `cargo test --release -p riscv-core` (and any other workspace crates you test).
- [ ] **Smoke test** — After building software (`make` in `software/`), run:  
  `./target/release/sim script scripts/tests/smoke_test.py`
- [ ] **Optional: full tests** — Run `hardware` test suite as needed.

## Documentation

- [ ] **docs/** — Up to date (architecture, API, getting started). No broken internal links.
- [ ] **File-level docs** — Critical modules and public APIs have doc comments where you want them.
- [ ] **scripts/README.md** — Matches current script layout and commands.


- **CI** — Add `.github/workflows/ci.yml` (or equivalent) to run `cargo build --release`, `cargo test -p riscv-core`, and optionally the smoke test.
