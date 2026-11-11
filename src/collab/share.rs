//! What a Live Share session is allowed to send to guests.
//!
//! The filter is authoritative on the host and used in two places: the file
//! list a guest sees on join, and the guard on every `Open` request. Hiding a
//! file from the list is not enough on its own — a guest can ask for any path
//! under the shared folder, so the same rules have to refuse the request too.

use crate::files::SKIP_DIRS;

/// Folders that hold machine state or secrets rather than source.
const DENIED_DIRS: &[&str] = &[".aws", ".ssh"];

/// Split a relative path into components, or `None` when it is not a path we
/// are willing to resolve at all.
///
/// This is the security-critical half. On Windows `\Users\me\.git\config` is a
/// *rooted* path: joining it onto the shared folder silently drops the folder
/// and the guest reads anywhere on the disk. Both separators are treated as
/// separators here, a leading one is refused, and a name Windows would trim of
/// trailing dots or spaces is refused rather than resolved to something else.
pub fn safe_components(rel: &str) -> Option<Vec<String>> {
    if rel.is_empty() || rel.starts_with('/') || rel.starts_with('\\') || rel.contains(':') {
        return None;
    }
    let mut out = Vec::new();
    for raw in rel.split(['/', '\\']) {
        if raw.is_empty() || raw == "." || raw == ".." {
            return None;
        }
        if raw.trim_end_matches([' ', '.']) != raw {
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

fn denied_dir(comp: &str) -> bool {
    SKIP_DIRS.iter().any(|d| d.eq_ignore_ascii_case(comp)) || DENIED_DIRS.iter().any(|d| d.eq_ignore_ascii_case(comp))
}

/// Names that are never worth sending: dotfiles that hold credentials and
/// private keys by any common naming scheme. Compared lowercased, because
/// Windows resolves `.SSH/config` to the real `.ssh` directory.
fn denied_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    if n == ".env" || n.starts_with(".env.") || n == ".envrc" || n.starts_with(".envrc.") {
        return true;
    }
    if n.ends_with(".pem") || n.ends_with(".key") || n.ends_with(".p12") || n.ends_with(".pfx") || n.ends_with(".jks") || n.ends_with(".kdbx") {
        return true;
    }
    if n.starts_with("id_rsa") || n.starts_with("id_ed25519") || n.starts_with("id_ecdsa") || n.starts_with("id_dsa") {
        return true;
    }
    if n.starts_with("credentials") {
        return true;
    }
    matches!(n.as_str(), ".netrc" | ".npmrc" | ".pypirc" | ".git-credentials")
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
    let file = rel.rsplit(['/', '\\']).next().unwrap_or(rel);
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
        assert!(!is_shareable(r"..\x"));
        assert!(!is_shareable(r"a\..\b"));
        assert!(!is_shareable(r"\Users\me\.git\config"), "a rooted path escapes the shared folder on Windows");
        assert!(!is_shareable(r"C:\Windows\win.ini"));
        assert!(!is_shareable(r"node_modules\x\index.js"), "backslash form of a skipped dir");
        assert!(!is_shareable(r".ssh\config"));
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
        assert!(!is_shareable(".env "), "Windows drops the trailing space and opens .env");
        assert!(!is_shareable(".env."));
        assert!(!is_shareable(r".ssh \config"));
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
