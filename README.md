# cargo-hoist

**cargo-hoist** is a Rust command-line tool that automates the hoisting of shared dependency source information (version, git, or local path) from sub‑crate manifests in a workspace to the root `Cargo.toml` under the `[workspace.dependencies]` section. This tool streamlines dependency management in large workspaces by centralizing versioning and source details while preserving extra attributes (like features, optional flags, etc.) in the individual crates.

![Rust](https://img.shields.io/badge/rust-2021-blue)
![License: MIT](https://img.shields.io/badge/license-MIT-green)

## Features

- **Centralize Dependency Sources:**  
  Automatically gathers dependency source information from multiple workspace members and hoists it to the root manifest.

- **Supports Multiple Dependency Types:**  
  Handles versioned dependencies, git dependencies (with branch, rev, or tag), and local path dependencies. Local paths are recalculated relative to the workspace root.

- **Interactive Conflict Resolution:**  
  If the same dependency is declared with conflicting source information across crates, the tool prompts you to select the correct source.

- **Preserves Extra Attributes:**  
  While the root manifest only stores the dependency source (version/git/path), extra attributes (features, optional, etc.) remain in the sub‑crate manifests with an added `workspace = true` flag.

- **Skips Already-Hoisted Dependencies:**  
  Dependencies that are already marked with a workspace import (`{ workspace = true }`) are automatically ignored.

- **Debug Logging:**  
  Built-in debug logging (using the `log` and `env_logger` crates) helps trace the tool’s behavior and diagnose issues.

## Installation

To install **cargo-hoist**, clone the repository and build the project with Cargo:

```bash
git clone https://github.com/yourusername/cargo-hoist.git
cd cargo-hoist
cargo install --path .
```

After installation, the tool is available as a Cargo subcommand:

```bash
cargo hoist -- /path/to/your/workspace
```

If no workspace path is specified, the current directory is assumed to be the workspace root.

## Usage

1. **Workspace Setup:**  
   Ensure your workspace root contains a `Cargo.toml` with a `[workspace]` section listing all member crates:

   ```toml
   [workspace]
   members = [
       "crate_a",
       "crate_b",
       // etc.
   ]
   ```

2. **Run cargo-hoist:**  
   From your workspace root (or by specifying the path), run:

   ```bash
   cargo hoist
   ```

   or

   ```bash
   cargo hoist -- /path/to/your/workspace
   ```

3. **Resolve Conflicts (if any):**  
   If the tool detects conflicting source specifications for a dependency, it will display the options and prompt you to choose one or skip hoisting that dependency.

4. **Review Changes:**  
   The tool updates:
   - The workspace root’s `Cargo.toml`, adding hoisted dependencies under `[workspace.dependencies]`.
   - Each sub‑crate’s `Cargo.toml`, replacing the dependency’s source keys (e.g., version/git/path) with `{ workspace = true }` while preserving extra attributes.

## Example

For instance, suppose you have the following in one of your crates:

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

The debug output will include information about dependency processing, computed relative paths, and updates performed on each manifest.

## Contributing

Contributions, bug reports, and feature requests are welcome! Feel free to open an issue or submit a pull request on the [GitHub repository](https://github.com/yourusername/cargo-hoist).

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for more details.

---

This README should help users quickly understand the purpose and usage of **cargo-hoist** and aid discoverability among developers managing large Rust workspaces. Enjoy streamlined dependency management!
