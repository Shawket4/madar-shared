# Claude Code Conventions: madar-shared

I/O-free Rust crates shared by the backend (MadarRust) and the POS core (madar/rust-core), consumed by git tag. Read README.md for what lives here and how releases work.

## Building
- **Faster builds: always `. scripts/dev-env.sh` first** in the shell that runs cargo (agents included). It turns on sccache (a compile cache shared by every worktree and agent) and, on Linux, the mold linker (much faster links of the big test binaries). Each part activates only when its tool is installed, so it is safe everywhere. Install once per machine: `cargo install --locked sccache`; Linux also `apt install mold clang`. Several worktrees or agents building at once: also give each its own `CARGO_TARGET_DIR`; sccache then shares the compiled dependencies between them. Never put the wrapper or linker in `.cargo/config.toml`: a machine or CI image without them would stop building.
