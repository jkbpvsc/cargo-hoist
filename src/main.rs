use std::{
    collections::HashMap,
    env,
    error::Error,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use log::{debug, warn};
use pathdiff::diff_paths;
use toml_edit::{value, DocumentMut, Formatted, InlineTable, Item, Table, TomlError, Value};

/// Represents the “source” of a dependency.
#[derive(Debug, Clone, PartialEq, Eq)]
enum DepSource {
    /// A version dependency (e.g. `"0.8.3"` or `{ version = "0.8.3", ... }`)
    Version(String),
    /// A git dependency with a URL and optionally branch/rev/tag.
    Git {
        url: String,
        branch: Option<String>,
        rev: Option<String>,
        tag: Option<String>,
    },
    /// A local path dependency. The string holds the path relative to the workspace root.
    Path(String),
    Workspace,
}

impl std::fmt::Display for DepSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DepSource::Version(v) => write!(f, "version: {}", v),
            DepSource::Git {
                url,
                branch,
                rev,
                tag,
            } => {
                write!(f, "git: {}", url)?;
                if let Some(branch) = branch {
                    write!(f, ", branch: {}", branch)?;
                }
                if let Some(rev) = rev {
                    write!(f, ", rev: {}", rev)?;
                }
                if let Some(tag) = tag {
                    write!(f, ", tag: {}", tag)?;
                }
                Ok(())
            }
            DepSource::Path(rel) => write!(f, "path: {}", rel),
            DepSource::Workspace => write!(f, "workspace"),
        }
    }
}

/// Given a dependency item from a Cargo.toml, compute its source information.
/// For local path dependencies, compute the path relative to the workspace root.
/// `cargo_toml_path` is the path to the sub‑crate’s Cargo.toml.
fn compute_dep_source(
    item: &Item,
    cargo_toml_path: &Path,
    workspace_root: &Path,
) -> Option<DepSource> {
    // If the item is a bare string, treat it as a version dependency.
    debug!("Checking item: {:?}", item);
    if let Some(val) = item.as_value() {
        if let Some(s) = val.as_str() {
            debug!("Found bare string dependency source: {}", s);
            return Some(DepSource::Version(s.to_string()));
        }
    }
    // If the item is a table:
    if let Some(table) = item.as_inline_table() {
        debug!("Found table dependency source: {:?}", table);
        // Check for a git dependency.
        if let Some(git_item) = table.get("git") {
            if let Some(git_url) = git_item.as_str() {
                let branch = table
                    .get("branch")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let rev = table
                    .get("rev")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let tag = table
                    .get("tag")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                return Some(DepSource::Git {
                    url: git_url.to_string(),
                    branch,
                    rev,
                    tag,
                });
            }
        } else if let Some(path_item) = table.get("path") {
            // Handle a local path dependency.
            if let Some(path_str) = path_item.as_str() {
                let crate_dir = cargo_toml_path.parent().unwrap_or_else(|| Path::new("."));
                // Compute the absolute path of the dependency.
                let abs_path = crate_dir.join(path_str).canonicalize().ok()?;
                // Canonicalize the workspace root too.
                let canonical_workspace = workspace_root.canonicalize().ok()?;
                let rel_path = diff_paths(&abs_path, &canonical_workspace)?;
                let mut rel_path_str = rel_path.to_string_lossy().to_string();
                // Prepend "./" if the result is not absolute and doesn't already start with "." or ".."
                if !rel_path_str.starts_with("./") && !rel_path_str.starts_with("../") {
                    rel_path_str = format!("./{}", rel_path_str);
                }
                debug!(
                    "Found local path dependency. Absolute: {:?}, Relative: {}",
                    abs_path, rel_path_str
                );
                return Some(DepSource::Path(rel_path_str));
            }
        } else if let Some(version_item) = table.get("version") {
            // Otherwise, if there is a version key, use that.
            if let Some(version_str) = version_item.as_str() {
                debug!("Found version dependency: {}", version_str);
                return Some(DepSource::Version(version_str.to_string()));
            }
        } else if let Some(workspace_item) = table.get("workspace") {
            if let Some(workspace_bool) = workspace_item.as_bool() {
                if workspace_bool {
                    debug!("Found workspace dependency");
                    return Some(DepSource::Workspace);
                }
            }
        }
    }

    warn!("Could not determine source for dependency item: {:?}", item);
    None
}

/// Build the workspace dependency item from a chosen dependency source.
/// The returned table will contain only the source information.
fn build_workspace_dep(dep_source: &DepSource) -> Item {
    let mut table = InlineTable::new();
    match dep_source {
        DepSource::Version(v) => {
            return Item::Value(Value::String(Formatted::new(v.clone())));
        }
        DepSource::Git {
            url,
            branch,
            rev,
            tag,
        } => {
            table.insert("git", Value::String(Formatted::new(url.clone())));
            if let Some(branch) = branch {
                table.insert("branch", Value::String(Formatted::new(branch.clone())));
            }
            if let Some(rev) = rev {
                table.insert("rev", Value::String(Formatted::new(rev.clone())));
            }
            if let Some(tag) = tag {
                table.insert("tag", Value::String(Formatted::new(tag.clone())));
            }
        }
        DepSource::Path(rel_path) => {
            table.insert("path", Value::String(Formatted::new(rel_path.clone())));
        }
        DepSource::Workspace => {
            panic!("Workspace source should not be used as a workspace dependency");
        }
    }

    debug!("Building workspace dependency: {}", dep_source);
    Item::Value(Value::InlineTable(table))
}

static KEYS_TO_IGNORE: [&str; 7] = [
    "version",
    "git",
    "branch",
    "rev",
    "tag",
    "path",
    "workspace",
];

/// Update a sub‑crate dependency specification so that it drops its source keys
/// (version, git, branch, rev, tag, path) and instead marks it as using the workspace source,
/// while preserving extra attributes (like features, optional, etc.).
fn update_subcrate_dependency(original: &Item) -> Item {
    match original {
        Item::Table(table) => {
            let mut new_table = table.clone();
            new_table.remove("version");
            new_table.remove("git");
            new_table.remove("branch");
            new_table.remove("rev");
            new_table.remove("tag");
            new_table.remove("path");
            // Use the proper boolean syntax.
            new_table["workspace"] = value(true);
            debug!("Updated sub-crate dependency table: {:?}", new_table);
            Item::Table(new_table)
        }
        Item::Value(Value::InlineTable(inline)) => {
            let mut new_inline_table = InlineTable::new();
            new_inline_table.insert("workspace", Value::Boolean(Formatted::new(true)));

            for (key, value) in inline.iter() {
                if KEYS_TO_IGNORE.contains(&key) {
                    continue;
                }
                new_inline_table.insert(key, value.clone());
            }

            debug!(
                "Updated sub-crate dependency inline: {:#?}",
                new_inline_table
            );
            Item::Value(Value::InlineTable(new_inline_table))
        }
        _ => {
            let mut inline = InlineTable::default();
            inline.insert("workspace", Value::Boolean(Formatted::new(true)));
            debug!("Updated sub-crate dependency inline: {:?}", inline);
            value(inline)
        }
    }
}

/// A CLI tool (suggested name: `cargo-hoist`) that walks a Rust workspace,
/// finds shared dependencies (those declared in multiple crates) and “hoists”
/// — their source information (version, git, or path) into the workspace root’s Cargo.toml under
/// `[workspace.dependencies]`. For dependencies with local paths, the tool updates the path to be relative
/// to the workspace root. Extra attributes (such as features, optional, etc.) remain in the sub‑crate manifests.
fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    // Determine the workspace root from the command-line argument or default to "."
    let workspace_dir = if let Some(arg1) = env::args().nth(1) {
        PathBuf::from(arg1)
    } else {
        env::current_dir()?
    };
    debug!("Workspace directory is: {:?}", workspace_dir);

    // Read and parse the root Cargo.toml
    let root_cargo = workspace_dir.join("Cargo.toml");
    debug!("Reading root Cargo.toml at: {:?}", root_cargo);
    let root_contents = fs::read_to_string(&root_cargo)
        .map_err(|e| format!("Could not read {}: {}", root_cargo.display(), e))?;
    let mut root_doc: DocumentMut = root_contents.parse().map_err(|e: TomlError| {
        format!(
            "Could not parse {} as TOML: {}",
            root_cargo.display(),
            e.to_string()
        )
    })?;

    // Get the workspace members from [workspace].members (as an array of strings)
    let workspace_table = root_doc
        .get("workspace")
        .and_then(Item::as_table)
        .ok_or("No [workspace] table found in root Cargo.toml")?;
    let members_array = workspace_table
        .get("members")
        .and_then(Item::as_array)
        .ok_or("No `members` array found in [workspace] of root Cargo.toml")?;
    debug!("Found {} members in the workspace", members_array.len());

    // Build the list of member Cargo.toml file paths.
    let mut member_paths = Vec::new();
    for member in members_array.iter() {
        if let Some(member_str) = member.as_str() {
            let member_cargo = workspace_dir.join(member_str).join("Cargo.toml");
            debug!("Adding member Cargo.toml: {:?}", member_cargo);
            member_paths.push(member_cargo);
        } else {
            warn!("Warning: skipping a non-string member entry: {}", member);
        }
    }

    // Map each dependency name to a vector of occurrences.
    // Each occurrence is (path-to-Cargo.toml, dependency specification, computed DepSource).
    let mut dep_occurrences: HashMap<String, Vec<(PathBuf, Item, DepSource)>> = HashMap::new();

    // First pass: read each package Cargo.toml and record its [dependencies].
    // **Skip any dependency that is already a workspace import.**
    for member_path in &member_paths {
        debug!("Processing member {:?}", member_path);
        let contents = fs::read_to_string(member_path)
            .map_err(|e| format!("Could not read {}: {}", member_path.display(), e))?;
        let doc: DocumentMut = contents.parse().map_err(|e: TomlError| {
            format!(
                "Could not parse {} as TOML: {}",
                member_path.display(),
                e.to_string()
            )
        })?;
        if let Some(deps) = doc.get("dependencies").and_then(Item::as_table) {
            for (dep_name, dep_value) in deps.iter() {
                debug!("Checking dependency `{}`", dep_name);
                // Skip dependencies that already use the workspace import.
                if let Some(table) = dep_value.as_table() {
                    if table
                        .get("workspace")
                        .and_then(|ws| ws.as_value().and_then(|v| v.as_bool()))
                        == Some(true)
                    {
                        debug!(
                            "Skipping {} in {:?} (already workspace)",
                            dep_name, member_path
                        );
                        continue;
                    }
                }
                if let Some(dep_source) = compute_dep_source(dep_value, member_path, &workspace_dir)
                {
                    if matches!(dep_source, DepSource::Workspace) {
                        debug!(
                            "Skipping {} in {:?} (already workspace)",
                            dep_name, member_path
                        );
                        continue;
                    };
                    dep_occurrences
                        .entry(dep_name.to_string())
                        .or_default()
                        .push((member_path.clone(), dep_value.clone(), dep_source));
                } else {
                    warn!(
                        "Warning: Could not determine source for dependency `{}` in {}. Skipping.",
                        dep_name,
                        member_path.display()
                    );
                }
            }
        }
    }

    // Determine shared dependencies: those that appear in more than one package.
    // For each, if there are conflicting source specifications, ask the user to choose one.
    let mut shared_deps: HashMap<String, DepSource> = HashMap::new();
    for (dep_name, occurrences) in dep_occurrences {
        debug!(
            "Dependency `{}` appears in {} members",
            dep_name,
            occurrences.len()
        );
        // Collect unique DepSource values.
        let mut source_options: Vec<DepSource> = Vec::new();
        for (_, _, dep_source) in &occurrences {
            if !source_options.contains(dep_source) {
                source_options.push(dep_source.clone());
            }
        }
        if source_options.is_empty() {
            continue;
        }
        if source_options.len() == 1 {
            // All occurrences agree.
            shared_deps.insert(dep_name, source_options[0].clone());
        } else {
            // Conflicting sources found. Ask the user to choose one.
            println!(
                "Dependency `{}` has conflicting source specifications:",
                dep_name
            );
            for (i, source) in source_options.iter().enumerate() {
                println!("  {}) {}", i + 1, source);
            }
            println!("  0) Skip hoisting this dependency");
            print!("Please choose an option for `{}` [0]: ", dep_name);
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();
            let choice: usize = if input.is_empty() {
                0
            } else {
                input.parse().unwrap_or(0)
            };
            if choice == 0 || choice > source_options.len() {
                debug!("Skipping hoisting dependency `{}`", dep_name);
            } else {
                shared_deps.insert(dep_name, source_options[choice - 1].clone());
            }
        }
    }

    // --- Update the workspace root Cargo.toml ---
    // Ensure that [workspace] and [workspace.dependencies] exist.
    if !root_doc.as_table().contains_key("workspace") {
        root_doc["workspace"] = Item::Table(Table::new());
    }
    if !root_doc["workspace"]
        .as_table()
        .unwrap()
        .contains_key("dependencies")
    {
        root_doc["workspace"]["dependencies"] = Item::Table(Table::new());
    }
    let workspace_deps = root_doc["workspace"]["dependencies"]
        .as_table_mut()
        .unwrap();

    // For each shared dependency, add it (with only its source information) to the workspace dependencies
    // if not already present.
    for (dep_name, dep_source) in &shared_deps {
        if !workspace_deps.contains_key(dep_name) {
            let workspace_item = build_workspace_dep(dep_source);
            workspace_deps[dep_name] = workspace_item;
            debug!(
                "Added shared dependency `{}` to workspace.dependencies in {}",
                dep_name,
                root_cargo.display()
            );
        }
    }

    // --- Update each member’s Cargo.toml ---
    // For each dependency that was hoisted, update the sub‑crate to reference it via a dependency spec
    // that drops the source keys while preserving extra attributes.
    for member_path in &member_paths {
        debug!("Updating dependencies in member {:?}", member_path);
        let contents = fs::read_to_string(member_path)
            .map_err(|e| format!("Could not read {}: {}", member_path.display(), e))?;
        let mut doc: DocumentMut = contents.parse().map_err(|e: TomlError| {
            format!(
                "Could not parse {} as TOML: {}",
                member_path.display(),
                e.to_string()
            )
        })?;
        let mut modified = false;
        if let Some(deps) = doc.get_mut("dependencies").and_then(Item::as_table_mut) {
            // Collect the keys before mutating.
            let keys: Vec<String> = deps.iter().map(|(k, _)| k.to_string()).collect();
            for key in keys {
                if shared_deps.contains_key(&key) {
                    // If the dependency already has a workspace import, skip updating.
                    if let Some(existing) = deps.get(&key) {
                        if let Some(tbl) = existing.as_table() {
                            if tbl
                                .get("workspace")
                                .and_then(|ws| ws.as_value().and_then(|v| v.as_bool()))
                                == Some(true)
                            {
                                debug!("Skipping {} in {:?} (already workspace)", key, member_path);
                                continue;
                            }
                        }
                    }
                    let original = deps[&key].clone();
                    let new_item = update_subcrate_dependency(&original);
                    deps[&key] = new_item;
                    modified = true;
                    debug!("Updated dependency `{}` in {:?}", key, member_path);
                }
            }
        }
        if modified {
            fs::write(member_path, doc.to_string())
                .map_err(|e| format!("Failed to write {}: {}", member_path.display(), e))?;
            debug!("Written updated file for {:?}", member_path);
        }
    }

    // Write the updated workspace root Cargo.toml.
    fs::write(&root_cargo, root_doc.to_string())
        .map_err(|e| format!("Failed to write {}: {}", root_cargo.display(), e))?;
    debug!(
        "Updated workspace root Cargo.toml at {}",
        root_cargo.display()
    );

    Ok(())
}
