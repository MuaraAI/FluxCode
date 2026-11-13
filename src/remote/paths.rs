//! Windows paths that point inside a WSL2 distro, and the Linux paths they mean.
//!
//! Windows reaches a distro's files through `\\wsl.localhost\<distro>\...` (or the
//! older `\\wsl$\...`), so the file code needs no change. Only commands have to be
//! translated, and that is what `WslTarget` carries.

use std::path::{Path, PathBuf};

/// A folder inside a WSL2 distro.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WslTarget {
    pub distro: String,
    /// The folder as the distro sees it: `/home/me/proj`.
    pub linux_root: String,
    /// The same folder as Windows sees it: `\\wsl.localhost\Ubuntu\home\me\proj`.
    pub unc_root: PathBuf,
}

impl WslTarget {
    /// A target for a distro and a folder inside it.
    pub fn new(distro: &str, linux_root: &str) -> Self {
        Self { distro: distro.to_string(), linux_root: linux_root.to_string(), unc_root: unc_of(distro, linux_root) }
    }

    /// The target for a Windows path, when it points inside a distro.
    pub fn from_path(path: &Path) -> Option<Self> {
        let (distro, tail) = split(path)?;
        let linux_root = linux_of(&tail);
        let unc_root = unc_of(&distro, &linux_root);
        Some(Self { distro, linux_root, unc_root })
    }

    /// The Linux path of a Windows path inside this target.
    pub fn linux_path(&self, path: &Path) -> Option<String> {
        let (distro, tail) = split(path)?;
        distro.eq_ignore_ascii_case(&self.distro).then(|| linux_of(&tail))
    }

    /// The Windows path of a Linux path inside this target.
    pub fn unc_path(&self, linux: &str) -> PathBuf {
        unc_of(&self.distro, linux)
    }
}

/// `\\wsl.localhost\Ubuntu\home\me` -> ("Ubuntu", "home\me").
/// Also accepts `\\wsl$\...` and the `\\?\UNC\...` form `canonicalize()` returns.
fn split(path: &Path) -> Option<(String, String)> {
    let s = path.to_string_lossy();
    let s = match s.strip_prefix(r"\\?\UNC\") {
        Some(rest) => format!(r"\\{rest}"),
        None => s.into_owned(),
    };
    let rest = s.strip_prefix(r"\\")?;
    let (host, after) = rest.split_once('\\').unwrap_or((rest, ""));
    if !host.eq_ignore_ascii_case("wsl.localhost") && !host.eq_ignore_ascii_case("wsl$") {
        return None;
    }
    let (distro, tail) = after.split_once('\\').unwrap_or((after, ""));
    if distro.is_empty() {
        return None;
    }
    Some((distro.to_string(), tail.to_string()))
}

/// A Windows tail (`home\me`) as a Linux path (`/home/me`); empty means the root.
fn linux_of(tail: &str) -> String {
    let tail = tail.trim_matches('\\');
    if tail.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", tail.replace('\\', "/"))
    }
}

fn unc_of(distro: &str, linux: &str) -> PathBuf {
    let tail = linux.trim_matches('/').replace('/', "\\");
    if tail.is_empty() {
        PathBuf::from(format!(r"\\wsl.localhost\{distro}"))
    } else {
        PathBuf::from(format!(r"\\wsl.localhost\{distro}\{tail}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_unc_forms() {
        for p in [r"\\wsl.localhost\Ubuntu\home\me\proj", r"\\wsl$\Ubuntu\home\me\proj", r"\\?\UNC\wsl.localhost\Ubuntu\home\me\proj"] {
            let t = WslTarget::from_path(Path::new(p)).expect(p);
            assert_eq!(t.distro, "Ubuntu");
            assert_eq!(t.linux_root, "/home/me/proj");
            assert_eq!(t.unc_root, PathBuf::from(r"\\wsl.localhost\Ubuntu\home\me\proj"));
        }
    }

    #[test]
    fn the_distro_root_is_slash() {
        let t = WslTarget::from_path(Path::new(r"\\wsl.localhost\Ubuntu")).unwrap();
        assert_eq!(t.linux_root, "/");
        assert_eq!(t.unc_path("/"), PathBuf::from(r"\\wsl.localhost\Ubuntu"));
    }

    #[test]
    fn a_windows_drive_is_not_wsl() {
        assert!(WslTarget::from_path(Path::new(r"C:\Users\me\proj")).is_none());
        assert!(WslTarget::from_path(Path::new(r"\\server\share\proj")).is_none());
    }

    #[test]
    fn a_new_target_builds_its_unc_root() {
        let t = WslTarget::new("Ubuntu", "/home/me/proj");
        assert_eq!(t.unc_root, PathBuf::from(r"\\wsl.localhost\Ubuntu\home\me\proj"));
        assert_eq!(WslTarget::new("Ubuntu", "/").unc_root, PathBuf::from(r"\\wsl.localhost\Ubuntu"));
    }

    #[test]
    fn paths_round_trip() {
        let t = WslTarget::from_path(Path::new(r"\\wsl.localhost\Ubuntu\home\me")).unwrap();
        assert_eq!(t.linux_path(Path::new(r"\\wsl.localhost\Ubuntu\home\me\a\b.rs")).as_deref(), Some("/home/me/a/b.rs"));
        assert_eq!(t.unc_path("/home/me/a/b.rs"), PathBuf::from(r"\\wsl.localhost\Ubuntu\home\me\a\b.rs"));
        // Another distro is not this target.
        assert!(t.linux_path(Path::new(r"\\wsl.localhost\Debian\home\me")).is_none());
    }
}
