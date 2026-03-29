//! `calepin extra skill`: install the calepin agent skill for coding assistants.
//!
//! Writes the embedded skill files to a data directory, then symlinks (Unix)
//! or copies (Windows) them into each selected tool's skill directory.

use anyhow::{Context, Result};
use include_dir::{include_dir, Dir};
use std::path::{Path, PathBuf};

static SKILL_FILES: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/scaffold/skill");

struct Tool {
    name: &'static str,
    personal_dir: &'static str, // relative to $HOME
    project_dir: &'static str,  // relative to cwd
}

const TOOLS: &[Tool] = &[
    Tool { name: "Claude",   personal_dir: ".claude/skills",   project_dir: ".claude/skills" },
    Tool { name: "Codex",    personal_dir: ".codex/skills",    project_dir: ".codex/skills" },
    Tool { name: "opencode", personal_dir: ".opencode/skills", project_dir: ".opencode/skills" },
    Tool { name: "pi",       personal_dir: ".pi/agent/skills/pi-skills", project_dir: ".pi/skills" },
];

fn home_dir() -> Result<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .context("cannot determine home directory (neither HOME nor USERPROFILE is set)")
}

fn data_dir(home: &Path) -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        return PathBuf::from(xdg);
    }
    #[cfg(target_os = "macos")]
    { home.join(".local/share") }
    #[cfg(target_os = "windows")]
    {
        std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join("AppData/Local"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    { home.join(".local/share") }
}

pub fn handle_skill(project: bool, claude: bool, codex: bool, opencode: bool, pi: bool) -> Result<()> {
    let home = home_dir()?;

    // 1. Write skill files to canonical location
    let canonical = data_dir(&home).join("calepin").join("skill");
    write_skill_files(&canonical)?;
    eprintln!("Wrote skill files to {}", canonical.display());

    // 2. Determine which tools to install for (none selected = all)
    let flags = [claude, codex, opencode, pi];
    let all = !flags.iter().any(|&f| f);

    // 3. Link or copy into each tool's skill directory
    for (tool, &selected) in TOOLS.iter().zip(flags.iter()) {
        if !all && !selected {
            continue;
        }

        let base = if project {
            std::env::current_dir().context("cannot determine current directory")?
        } else {
            home.clone()
        };

        let skill_dir = if project {
            base.join(tool.project_dir)
        } else {
            base.join(tool.personal_dir)
        };

        let target_path = skill_dir.join("calepin");

        // Remove existing entry
        if target_path.is_symlink() || target_path.exists() {
            if target_path.is_symlink() || target_path.is_file() {
                std::fs::remove_file(&target_path).ok();
            } else {
                std::fs::remove_dir_all(&target_path).ok();
            }
        }

        std::fs::create_dir_all(&skill_dir)
            .with_context(|| format!("cannot create {}", skill_dir.display()))?;

        link_or_copy(&canonical, &target_path)?;

        let mode = if project { "project" } else { "personal" };
        eprintln!("  {} {} -> {}", tool.name, mode, target_path.display());
    }

    Ok(())
}

/// Try to symlink; fall back to copying the directory tree.
fn link_or_copy(src: &Path, dst: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(src, dst)
            .with_context(|| format!("cannot symlink {} -> {}", dst.display(), src.display()))?;
        return Ok(());
    }
    #[cfg(not(unix))]
    {
        copy_dir(src, dst)
    }
}

#[cfg(not(unix))]
fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dest = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

fn write_skill_files(dir: &Path) -> Result<()> {
    if dir.exists() {
        std::fs::remove_dir_all(dir).ok();
    }
    std::fs::create_dir_all(dir)?;
    extract_dir(&SKILL_FILES, dir)
}

fn extract_dir(embedded: &Dir<'_>, root: &Path) -> Result<()> {
    for file in embedded.files() {
        let name = file.path().file_name().unwrap();
        let dest = root.join(name);
        std::fs::write(&dest, file.contents())?;
    }
    for sub in embedded.dirs() {
        let sub_name = sub.path().file_name().unwrap();
        let sub_dest = root.join(sub_name);
        std::fs::create_dir_all(&sub_dest)?;
        extract_dir(sub, &sub_dest)?;
    }
    Ok(())
}
