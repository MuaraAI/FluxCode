//! Cross-platform system abstraction layer.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    #[test]
    #[cfg(unix)]
    fn test_normalize_path_unix() {
        assert_eq!(normalize_path("/home/user/project/"), "/home/user/project");
        assert_eq!(normalize_path("/home/User/Project"), "/home/User/Project"); // case preserved
        assert_eq!(normalize_path("/"), "/");
    }

    #[test]
    #[cfg(unix)]
    fn test_is_path_under_unix() {
        assert!(is_path_under("/home/user/project/src", "/home/user/project"));
        assert!(is_path_under("/home/user/project", "/home/user/project"));
        assert!(!is_path_under("/home/user/project2", "/home/user/project"));
        assert!(!is_path_under("/home/user/other", "/home/user/project"));
    }

    #[test]
    #[cfg(unix)]
    fn test_system_roots_unix() {
        let roots = get_system_roots();
        assert_eq!(roots, vec!["/".to_string()]);
    }

    #[test]
    #[cfg(unix)]
    fn test_platform_name_unix() {
        assert_eq!(get_platform(), "linux");
    }

    #[test]
    #[cfg(unix)]
    fn test_resolve_venv_python_unix() {
        let tmp = std::env::temp_dir().join(format!("fluxcode_test_venv_{}", std::process::id()));
        let venv_bin = tmp.join(".venv").join("bin");
        fs::create_dir_all(&venv_bin).unwrap();
        let py_bin = venv_bin.join("python");
        fs::write(&py_bin, "#!/bin/sh\necho ok").unwrap();

        let resolved = resolve_venv_python(&tmp);
        let _ = fs::remove_dir_all(&tmp);
        assert_eq!(resolved, Some(py_bin));
    }
}
