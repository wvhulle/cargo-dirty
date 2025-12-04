# cargo-dirty

This tool tells the cause of long Rust / Cargo recompilations. It uses verbose output of the Cargo `check` command for this and may detect:

- environment variable changes (most common)
- file changes
- Rust compiler flags changed
- dependency changed
- feature changed
- ...

## Usage

Just run this command:

```bash
cargo dirty
```

which will print something like:

```bash
Running: cargo check

1 root cause:
  cargo-dirty [cargo-dirty] file:src/main.rs
```

The root cause of the rebuild is shown in the terminal.

You can also use the `--json` flag for structured output.

## Installation

Installation:

```bash
cargo install cargo-dirty
```

 If something does not work for you, please create a bug report in the source repository.
