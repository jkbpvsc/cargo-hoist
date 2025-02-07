# cargo-hoist

**cargo-hoist** is a CLI tool that automatically hoists shared dependency sources (version, git, or local path) from Rust workspace crates into the root `Cargo.toml` under the `[workspace.dependencies]` section. This centralizes dependency source information while preserving extra attributes (such as features and optional flags) in the individual crate manifests.

![Rust](https://img.shields.io/badge/rust-2021-blue)
![License: MIT](https://img.shields.io/badge/license-MIT-green)

## Features

- **Centralize Dependency Sources:**  
  Automatically hoist version, git, or local path information from multiple workspace members into the root manifest.

- **Supports Multiple Dependency Types:**  
  Handles versioned, git, and local path dependencies. Local paths are recalculated relative to the workspace root.

- **Interactive Conflict Resolution:**  
  When a dependency is declared with conflicting source information across crates, cargo-hoist prompts you to choose the correct source.

- **Preserves Extra Attributes:**  
  Extra attributes (like features, optional flags, etc.) remain in the sub‑crate manifests while the hoisted dependency in the root only stores the source information.

- **Skips Already-Hoisted Dependencies:**  
  Dependencies already marked with `{ workspace = true }` are automatically ignored.

- **Debug Logging:**  
  Built-in debug logging (using the `log` and `env_logger` crates) helps trace the tool’s actions and diagnose issues.

## Installation

### Install from GitHub

To install **cargo-hoist** directly from GitHub, run:

```bash
cargo install --git https://github.com/jkbpvsc/cargo-hoist.git
```

This will build and install the tool so you can invoke it as a Cargo subcommand:

```bash
cargo hoist [/path/to/your/workspace]
```

If no workspace path is specified, the current directory is assumed to be the workspace root.

## Usage

1. **Workspace Setup:**  
   Ensure your workspace root contains a `Cargo.toml` with a `[workspace]` section listing all member crates. For example:

   ```toml
   [workspace]
   members = [
       "crate_a",
       "crate_b",
       // etc.
   ]
   ```

2. **Run cargo-hoist:**  
   From your workspace root (or by specifying the workspace path), run:

   ```bash
   cargo hoist
   ```

   or

   ```bash
   cargo hoist /path/to/your/workspace
   ```

3. **Resolve Conflicts:**  
   If the same dependency appears in multiple crates with conflicting source specifications, cargo-hoist will prompt you to select the correct source or to skip hoisting that dependency.

4. **Review Changes:**  
   After running, cargo-hoist updates:
   - The workspace root’s `Cargo.toml` by adding hoisted dependencies under `[workspace.dependencies]`.
   - Each sub‑crate’s `Cargo.toml` by replacing the dependency’s source keys (e.g., version, git, or path) with `{ workspace = true }`, while preserving extra attributes.

### Example

For instance, suppose you have the following dependency in one of your crates:

```toml
[dependencies]
tonic = { version = "0.8.3", features = ["tls", "tls-roots", "tls-webpki-roots"] }
```

After running **cargo-hoist**, if this dependency is shared among multiple crates, the tool will hoist the source information to the workspace root:

**Workspace `Cargo.toml`:**

```toml
[workspace.dependencies]
tonic = { version = "0.8.3" }
```

And update each sub‑crate’s manifest accordingly:

```toml
[dependencies]
tonic = { workspace = true, features = ["tls", "tls-roots", "tls-webpki-roots"] }
```

## Debugging

To see detailed debug logs when running **cargo-hoist**, set the `RUST_LOG` environment variable before executing the tool:

```bash
RUST_LOG=debug cargo hoist
```

The debug output includes information about dependency processing, computed relative paths for local dependencies, and file update actions.

## Contributing

Contributions, bug reports, and feature requests are welcome! Feel free to open an issue or submit a pull request on the [GitHub repository](https://github.com/jkbpvsc/cargo-hoist).

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
```
