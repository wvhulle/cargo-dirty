# cargo-dirty

When you run into the issue that Cargo compiles take a long time, you may want to use this tool. This tools gives detailed feedback what the cause is of long recompilations. Often it is a hidden change of environment variables.

Installation:

1. Install `rustup`
2. Clone this repo
3. Run `cargo install --path .`

Or without manual cloning, run `cargo install --git [GITHUB_REPO_URL]`

Then to use this tool:

```bash
cargo dirty
# alternatively,
cargo-dirty
```

It should show some output in the terminal containing the reasons (if any) of Cargo recompiles.
