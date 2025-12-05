# cargo-frequent

This tool tells the cause of frequent recompilations by Cargo in Rust projects. It uses by default the verbose output of the Cargo `check` command. Causes of frequent recompiles that this tool may find:

- environment variable changes (most common)
- file changes
- Rust compiler flags changed
- dependency changed
- feature changed
- ...

## Usage

Just run this command:

```bash
cargo frequent
```

which will print something like:

```bash
Running: cargo check

1 root cause:
  cargo-frequent [cargo-frequent] file:src/main.rs
```

The root cause of the rebuild is shown in the terminal.

You can also use the `--json` flag for structured output.

## Installation

Installation:

```bash
cargo install cargo-frequent
```

 If something does not work for you, please create a bug report in the source repository.
