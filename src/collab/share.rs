//! What a Live Share session is allowed to send to guests.
//!
//! The filter is authoritative on the host and used in two places: the file
//! list a guest sees on join, and the guard on every `Open` request. Hiding a
//! file from the list is not enough on its own — a guest can ask for any path
//! under the shared folder, so the same rules have to refuse the request too.

use crate::files::SKIP_DIRS;

/// Folders that hold machine state or secrets rather than source.
const DENIED_DIRS: &[&str] = &[".aws", ".ssh"];

/// Both separators count as separators on Windows, where a guest can send either.
/// On Linux and macOS `\` is an ordinary character in a file name, so treating it
/// as a separator would make such a file unreachable rather than unsafe.
fn is_sep(c: char) -> bool {
    c == '/' || (cfg!(windows) && c == '\\')
}

/// Split a relative path into components, or `None` when it is not a path we
/// are willing to resolve at all.
///
/// This is the security-critical half. On Windows `\Users\me\.git\config` is a
/// *rooted* path: joining it onto the shared folder drops the folder and the
/// guest reads anywhere on the disk, so a leading separator, a drive letter and a
/// component Windows would trim of trailing dots or spaces are all refused
/// rather than resolved to something else.
pub fn safe_components(rel: &str) -> Option<Vec<String>> {
    if rel.is_empty() {
        return None;
    }
    let bytes = rel.as_bytes();
    if rel.starts_with('/') || (cfg!(windows) && bytes.first() == Some(&b'\\')) || rel.contains(':') {
        return None;
    }
    let mut out = Vec::new();
    for raw in rel.split(is_sep) {
        if raw.is_empty() || raw == "." || raw == ".." {
            return None;
        }
        if cfg!(windows) && raw.trim_end_matches([' ', '.']) != raw {
            return None;
        }
        out.push(raw.to_string());
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Would a file system that folds case resolve `name` to `denied`?
///
/// ASCII folding is not enough: NTFS's upcase table maps `ı` (U+0131) to `I` and
/// `ſ` (U+017F) to `S`, so `.gıt/config` and `.ſsh/config` open the real `.git`
/// and `.ssh`. Comparing both directions in full Unicode closes that, and costs
/// nothing for the ASCII names that make up the list.
fn folds_to(name: &str, denied: &str) -> bool {
    name.eq_ignore_ascii_case(denied) || name.to_uppercase() == denied.to_uppercase() || name.to_lowercase() == denied.to_lowercase()
}

fn denied_dir(comp: &str) -> bool {
    SKIP_DIRS.iter().any(|d| folds_to(comp, d)) || DENIED_DIRS.iter().any(|d| folds_to(comp, d))
}

/// Names that are never worth sending: dotfiles that hold credentials and
/// private keys by any common naming scheme. Compared with the same folding as
/// [`denied_dir`], because Windows resolves `.SSH/config` to the real directory.
fn denied_name(name: &str) -> bool {
    let starts = |p: &str| name.len() >= p.len() && folds_to(&name[..p.len()], p);
    let ends = |p: &str| name.len() >= p.len() && folds_to(&name[name.len() - p.len()..], p);
    if folds_to(name, ".env") || starts(".env.") || folds_to(name, ".envrc") || starts(".envrc.") {
        return true;
    }
    if [".pem", ".key", ".p12", ".pfx", ".jks", ".kdbx"].iter().any(|e| ends(e)) {
        return true;
    }
    if ["id_rsa", "id_ed25519", "id_ecdsa", "id_dsa", "credentials"].iter().any(|p| starts(p)) {
        return true;
    }
    [".netrc", ".npmrc", ".pypirc", ".git-credentials"].iter().any(|p| folds_to(name, p))
}

pub fn is_shareable(rel: &str) -> bool {
    let Some(parts) = safe_components(rel) else { return false };
    let Some((last, dirs)) = parts.split_last() else { return false };
    if dirs.iter().any(|d| denied_dir(d)) {
        return false;
    }
    !denied_name(last)
}

pub fn ensure_shareable(rel: &str) -> Result<(), String> {
    if is_shareable(rel) {
        return Ok(());
    }
    let file = rel.rsplit(is_sep).next().unwrap_or(rel);
    Err(crate::trf!("\"{file}\" can't be shared."; file => file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_normal_source() {
        assert!(is_shareable("src/main.rs"));
        assert!(is_shareable("README.md"));
        assert!(is_shareable(".gitignore"));
        assert!(is_shareable("docs/notes.md"));
    }

    #[test]
    fn refuses_skipped_dirs() {
        assert!(!is_shareable("node_modules/x/index.js"));
        assert!(!is_shareable(".git/config"));
        assert!(!is_shareable("target/a"));
        assert!(!is_shareable("dist/bundle.js"));
        assert!(!is_shareable(".venv/lib/py.py"));
    }

    #[test]
    fn refuses_env_and_key_files() {
        assert!(!is_shareable(".env"));
        assert!(!is_shareable(".env.local"));
        assert!(!is_shareable(".env.production"));
        assert!(!is_shareable("server.pem"));
        assert!(!is_shareable("certs/ca.key"));
        assert!(!is_shareable("id_rsa"));
        assert!(!is_shareable("keys/id_ed25519"));
        assert!(!is_shareable(".ssh/id_ed25519"));
        assert!(!is_shareable(".aws/credentials"));
        assert!(!is_shareable(".git-credentials"));
        assert!(!is_shareable(".npmrc"));
        assert!(!is_shareable("vault/db.kdbx"));
    }

    #[test]
    fn refuses_traversal() {
        assert!(!is_shareable("../x"));
        assert!(!is_shareable("/abs"));
        assert!(!is_shareable("a/../b"));
        assert!(!is_shareable("a//b"));
        assert!(!is_shareable("C:\\x"));
    }

    #[test]
    fn refuses_windows_separators_and_rooted_paths() {
        // A backslash is a separator only on Windows; on Linux it is an ordinary
        // character, and refusing it there would hide a real file.
        if cfg!(windows) {
            assert!(!is_shareable(r"..\x"));
            assert!(!is_shareable(r"a\..\b"));
            assert!(!is_shareable(r"\Users\me\.git\config"), "a rooted path escapes the shared folder");
            assert!(!is_shareable(r"node_modules\x\index.js"), "backslash form of a skipped dir");
            assert!(!is_shareable(r".ssh\config"));
        } else {
            assert!(is_shareable(r"weird\name.txt"), "a backslash is legal in a POSIX name");
        }
        // A drive letter and a drive-absolute path are refused everywhere.
        assert!(!is_shareable(r"C:\Windows\win.ini"));
        assert!(!is_shareable("C:/Windows/win.ini"));
    }

    #[test]
    fn refuses_denied_names_case_insensitively() {
        assert!(!is_shareable(".SSH/config"));
        assert!(!is_shareable(".AWS/credentials"));
        assert!(!is_shareable(".GIT/config"));
        assert!(!is_shareable("NODE_MODULES/x.js"));
        assert!(!is_shareable(".ENV"));
    }

    #[test]
    fn refuses_names_windows_would_trim() {
        if cfg!(windows) {
            assert!(!is_shareable(".env "), "Windows drops the trailing space and opens .env");
            assert!(!is_shareable(".env."));
            assert!(!is_shareable(r".ssh \config"));
        }
    }

    #[test]
    fn refuses_names_that_fold_to_a_denied_one() {
        // NTFS's upcase table maps these to ASCII: `.gıt` opens `.git`, `.ſsh`
        // opens `.ssh`. ASCII-only folding would let both through.
        assert!(!is_shareable(".gıt/config"), "dotless i uppercases to I on NTFS");
        assert!(!is_shareable(".ſsh/config"), "long s uppercases to S on NTFS");
        assert!(!is_shareable(".ssh/CONFIG"));
    }

    #[test]
    fn refuses_more_credential_names() {
        assert!(!is_shareable(".envrc"), "direnv holds cloud tokens");
        assert!(!is_shareable("credentials"));
        assert!(!is_shareable("credentials.json"), "GCP service account");
        assert!(!is_shareable("id_dsa"));
    }

    #[test]
    fn ensure_shareable_reports_the_file_name() {
        assert_eq!(ensure_shareable("src/main.rs"), Ok(()));
        assert_eq!(ensure_shareable(".env"), Err("\".env\" can't be shared.".to_string()));
        assert_eq!(ensure_shareable("node_modules/x"), Err("\"x\" can't be shared.".to_string()));
    }
}
