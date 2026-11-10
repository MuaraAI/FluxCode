//! What a Live Share session is allowed to send to guests.
//!
//! The filter is authoritative on the host and used in two places: the file
//! list a guest sees on join, and the guard on every `Open` request. Hiding a
//! file from the list is not enough on its own — a guest can ask for any path
//! under the shared folder, so the same rules have to refuse the request too.

use crate::files::SKIP_DIRS;

/// Folders that hold machine state or secrets rather than source.
const DENIED_DIRS: &[&str] = &[".aws", ".ssh"];

/// Whole names that are never worth sending: dotfiles that hold credentials
/// and private keys by any common naming scheme.
fn denied_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    if n == ".env" || n.starts_with(".env.") {
        return true;
    }
    if n.ends_with(".pem") || n.ends_with(".key") || n.ends_with(".p12") || n.ends_with(".pfx") || n.ends_with(".jks") || n.ends_with(".kdbx") {
        return true;
    }
    if n.starts_with("id_rsa") || n.starts_with("id_ed25519") || n.starts_with("id_ecdsa") {
        return true;
    }
    matches!(n.as_str(), ".netrc" | ".npmrc" | ".pypirc" | ".git-credentials")
}

pub fn is_shareable(rel: &str) -> bool {
    if rel.starts_with('/') || rel.contains(':') {
        return false;
    }
    let parts: Vec<&str> = rel.split('/').collect();
    if parts.iter().any(|p| p.is_empty() || *p == ".." || *p == ".") {
        return false;
    }
    if parts[..parts.len() - 1].iter().any(|p| SKIP_DIRS.contains(p) || DENIED_DIRS.contains(p)) {
        return false;
    }
    !denied_name(parts[parts.len() - 1])
}

pub fn ensure_shareable(rel: &str) -> Result<(), String> {
    if is_shareable(rel) {
        Ok(())
    } else {
        let file = rel.rsplit('/').next().unwrap_or(rel);
        Err(format!("\"{file}\" can't be shared."))
    }
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
    fn ensure_shareable_reports_the_file_name() {
        assert_eq!(ensure_shareable("src/main.rs"), Ok(()));
        assert_eq!(ensure_shareable(".env"), Err("\".env\" can't be shared.".to_string()));
        assert_eq!(ensure_shareable("node_modules/x"), Err("\"x\" can't be shared.".to_string()));
    }
}
