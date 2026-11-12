//! Parsing of "open this package" requests (double-click / Open With / second instance).
//! Pure functions so the rules are unit-tested independently of Tauri and of the operating system.
use std::path::{Path, PathBuf};

pub const PACKAGE_EXTENSION: &str = "parcel";

pub fn is_package_path(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case(PACKAGE_EXTENSION))
}

/// Extract package paths from process arguments.
///
/// * the first argument (the executable) is skipped when `skip_program` is set;
/// * flags (`-x`, `--y`) are ignored;
/// * only existing regular files ending in `.parcel` are accepted — never anything else, so a crafted
///   command line cannot make Parcel touch arbitrary paths;
/// * relative paths are resolved against `cwd`; duplicates are removed (case-insensitively).
pub fn package_paths_from_args<I, S>(args: I, cwd: Option<&Path>, skip_program: bool) -> Vec<PathBuf>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut out: Vec<PathBuf> = Vec::new();
    for (i, a) in args.into_iter().enumerate() {
        if skip_program && i == 0 {
            continue;
        }
        let a = a.as_ref().trim().trim_matches('"');
        if a.is_empty() || a.starts_with('-') || a.contains('\0') {
            continue;
        }
        let mut p = PathBuf::from(a);
        if !p.is_absolute() {
            match cwd {
                Some(c) => p = c.join(p),
                None => continue,
            }
        }
        if !is_package_path(&p) || !p.is_file() {
            continue;
        }
        let key = p.to_string_lossy().to_lowercase();
        if !out.iter().any(|x| x.to_string_lossy().to_lowercase() == key) {
            out.push(p);
        }
    }
    out
}

/// Windows canonicalisation yields verbatim paths (`\\?\C:\x`). Shell APIs that reveal a file in the file
/// manager do not accept them, so strip the prefix for display and reveal (UNC paths are left alone).
pub fn strip_verbatim_prefix(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => p.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbatim_prefix_is_stripped_only_for_drive_paths() {
        assert_eq!(strip_verbatim_prefix(Path::new(r"\\?\C:\Users\me\f.txt")), PathBuf::from(r"C:\Users\me\f.txt"));
        assert_eq!(strip_verbatim_prefix(Path::new(r"\\?\UNC\server\share\f")), PathBuf::from(r"\\?\UNC\server\share\f"));
        assert_eq!(strip_verbatim_prefix(Path::new("/home/me/f")), PathBuf::from("/home/me/f"));
        assert_eq!(strip_verbatim_prefix(Path::new(r"C:\plain")), PathBuf::from(r"C:\plain"));
    }

    #[test]
    fn extension_check_is_case_insensitive_and_exact() {
        assert!(is_package_path(Path::new("a/B.PaRcEl")));
        assert!(!is_package_path(Path::new("a/b.parcel.txt")));
        assert!(!is_package_path(Path::new("a/.parcel")));
        assert!(!is_package_path(Path::new("a/b.zip")));
    }
}
