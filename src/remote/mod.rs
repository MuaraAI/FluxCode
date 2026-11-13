//! A workspace that lives inside a WSL2 distro.
//!
//! Windows reaches the distro's files through `\\wsl.localhost\<distro>\...`, so
//! reading and writing them needs no new code. Commands, though, have to run
//! inside the distro, and that is what this module builds.

mod paths;
mod wsl;

pub use paths::WslTarget;
pub use wsl::{available, distros, home_dir, program_command, program_command_env, shell_command};
