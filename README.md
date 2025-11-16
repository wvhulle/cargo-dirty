# cargo-dirty

When you run into the issue that Cargo compiles take a long time, you may want to use this tool. This tools gives detailed feedback what the cause is of long recompilations. Often it is a hidden change of environment variables.

Installation:

```bash
cargo install cargo-dirty
```

Then to use this tool:

```bash
cargo dirty
```

It should show some output in the terminal containing the reasons (if any) of Cargo recompiles.
