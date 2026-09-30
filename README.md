# tftio-org

Typed org-mode AST and parser

## Getting started

Toolchain, task execution, and hook tools are managed by mise.
The Rust toolchain is declared in `mise.toml`; rustup is an implementation
detail and `rust-toolchain.toml` is intentionally absent.
Entering the directory does not prepare, install, or regenerate anything: setup
is explicit, so no lockfile ever changes because someone walked into the
repository.

```sh
mise trust --quiet
mise install
mise run setup:idea   # optional; regenerates the gitignored .idea/
```

Tools come from `mise activate <shell>` in an interactive shell, and from mise
shims for non-interactive processes such as editors and coding agents.

Dependencies move on one deliberate command, and never on their own:

```sh
mise run update       # mise tools, cargo crates, prek hooks
mise run check:locks  # read-only; fails if Cargo.lock is stale
```

## Tasks

```sh
mise run check  # check-only hooks, as CI runs them
mise run lint   # manual autofix hooks
mise run test   # test suite
mise run ci     # full CI gate
```

The generated Rust gate includes formatting, TOML formatting, shell linting,
spelling, clippy, nextest, docs, unused-dependency detection, advisory audit,
license/source policy, packaging, and a 95% line-coverage floor.
