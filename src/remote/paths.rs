
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WslTarget {
    pub distro: String,
    pub linux_root: String,
    pub unc_root: PathBuf,
}

impl WslTarget {
    pub fn new(distro: &str, linux_root: &str) -> Self {
        Self { distro: distro.to_string(), linux_root: linux_root.to_string(), unc_root: unc_of(distro, linux_root) }
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        let (distro, tail) = split(path)?;
        let linux_root = linux_of(&tail);
        let unc_root = unc_of(&distro, &linux_root);
        Some(Self { distro, linux_root, unc_root })
    }

    pub fn linux_path(&self, path: &Path) -> Option<String> {
        let (distro, tail) = split(path)?;
        distro.eq_ignore_ascii_case(&self.distro).then(|| linux_of(&tail))
    }

    pub fn unc_path(&self, linux: &str) -> PathBuf {
        unc_of(&self.distro, linux)
    }
}

fn split(path: &Path) -> Option<(String, String)> {
    let s = path.to_string_lossy().replace('/', "\\");
    let s = if let Some(rest) = crate::docs::strip_prefix_ignore_ascii_case(&s, r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else {
        s
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
    fn a_distro_name_may_have_dots_and_dashes() {
        for (p, distro) in [
            (r"\\wsl.localhost\Ubuntu-22.04\home\me\proj", "Ubuntu-22.04"),
            (r"\\wsl$\Ubuntu-22.04\home\me\proj", "Ubuntu-22.04"),
            (r"\\wsl.localhost\my.distro\home\me", "my.distro"),
            (r"\\wsl.localhost\Debian-12-bookworm\srv\app", "Debian-12-bookworm"),
        ] {
            let t = WslTarget::from_path(Path::new(p)).expect(p);
            assert_eq!(t.distro, distro, "{p}");
            assert_eq!(t.linux_path(Path::new(p)).as_deref(), Some(t.linux_root.as_str()), "{p}");
        }
        let t = WslTarget::from_path(Path::new(r"\\wsl.localhost\Ubuntu-22.04\home\me")).unwrap();
        assert_eq!(t.linux_root, "/home/me");
        assert_eq!(t.unc_path("/home/me"), PathBuf::from(r"\\wsl.localhost\Ubuntu-22.04\home\me"));
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
        assert_eq!(
            t.linux_path(Path::new(r"\\wsl.localhost\Ubuntu\home\me\a\b.rs"))
                .as_deref(),
            Some("/home/me/a/b.rs")
        );
        assert_eq!(
            t.unc_path("/home/me/a/b.rs"),
            PathBuf::from(r"\\wsl.localhost\Ubuntu\home\me\a\b.rs")
        );
        assert!(t
            .linux_path(Path::new(r"\\wsl.localhost\Debian\home\me"))
            .is_none());
    }

    #[test]
    fn forward_slashes_and_mixed_separators_are_accepted() {
        for p in [
            r"//wsl.localhost/Ubuntu/home/me/proj",
            r"\\wsl.localhost\Ubuntu/home/me/proj",
            r"//wsl$/Ubuntu/home/me/proj",
            r"//?/UNC/wsl.localhost/Ubuntu/home/me/proj",
            r"//?/unc/wsl.localhost/Ubuntu/home/me/proj",
        ] {
            let t = WslTarget::from_path(Path::new(p)).expect(p);
            assert_eq!(t.distro, "Ubuntu", "{p}");
            assert_eq!(t.linux_root, "/home/me/proj", "{p}");
            assert_eq!(
                t.unc_root,
                PathBuf::from(r"\\wsl.localhost\Ubuntu\home\me\proj"),
                "{p}"
            );
        }
    }

    #[test]
    fn trailing_slashes_and_distro_case_still_round_trip() {
        for p in [
            r"\\wsl.localhost\Ubuntu\home\me\\",
            r"//wsl.localhost/Ubuntu/home/me/",
        ] {
            let t = WslTarget::from_path(Path::new(p)).expect(p);
            assert_eq!(t.linux_root, "/home/me", "{p}");
        }
        let t = WslTarget::new("Ubuntu", "/home/me");
        assert_eq!(
            t.linux_path(Path::new(r"\\wsl.localhost\ubuntu\home\me\a.rs"))
                .as_deref(),
            Some("/home/me/a.rs")
        );
        assert_eq!(
            t.linux_path(Path::new(r"\\wsl$\Ubuntu\home\me\a.rs"))
                .as_deref(),
            Some("/home/me/a.rs")
        );
        assert_eq!(
            t.linux_path(Path::new(r"\\?\UNC\wsl.localhost\Ubuntu\home\me\a.rs"))
                .as_deref(),
            Some("/home/me/a.rs")
        );
        assert_eq!(
            t.linux_path(Path::new(r"\\?\UNC\wsl$\Ubuntu\home\me\a.rs"))
                .as_deref(),
            Some("/home/me/a.rs")
        );
        assert_eq!(
            t.unc_path("/home/me/my file.rs"),
            PathBuf::from(r"\\wsl.localhost\Ubuntu\home\me\my file.rs")
        );
    }
}
