use std::path::Path;

/// Same uid/gid as Pterodactyl's default `pterodactyl` user, the user every
/// server container runs as.
pub const CONTAINER_UID: u32 = 988;

/// Recursively chowns to the container user without following symlinks, like
/// Wings' `CheckPermissionsOnBoot`.
pub fn chown_all(root: &Path) -> std::io::Result<()> {
  for entry in walkdir::WalkDir::new(root) {
    let entry = entry?;
    std::os::unix::fs::lchown(entry.path(), Some(CONTAINER_UID), Some(CONTAINER_UID))?;
  }
  Ok(())
}

/// Bytes used under `root`, symlinks not followed.
pub fn disk_usage(root: &Path) -> u64 {
  walkdir::WalkDir::new(root)
    .into_iter()
    .filter_map(|e| e.ok()?.metadata().ok())
    .filter(|m| m.is_file())
    .map(|m| m.len())
    .sum()
}
