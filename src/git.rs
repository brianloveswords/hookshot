//! A library for interacting with the git cli.
//!
//! This library provides an interface for operating on git repository. It is
//! not intended to provide a 1-1 interface to the git cli but instead provide a
//! minimal interface to create the smallest checkout for a specific sha.

use std::process::{Command, Output};
use std::path::Path;
use ::verified_path::directory_exists;

use ::error::CommandError;

pub struct Git<'a> {
    /// Remote path to the repository. This can be a filesystem path if the
    /// `file://` protocol is used.
    pub remote_path: String,

    /// Local path of where to clone the repository.
    pub local_path: &'a Path,

    /// Branch to check out. We require this so we can create the smallest
    /// checkout possible.
    pub branch: String,
}

impl<'a> Git<'a> {
    fn clone(&self) -> Result<Output, CommandError> {
        let result = match Command::new("git")
            .arg("clone")
            .arg("--depth=1")
            .arg("--single-branch")
            .arg("-b").arg(&self.branch)
            .arg(&self.remote_path)
            .arg(self.local_path)
            .output() {
                Ok(r) => r,
                Err(e) => return Err(CommandError {
                    desc: "failed to execute process, see detail",
                    output: None,
                    detail: Some(format!("{}", e)),
                })
            };

        match result.status.success() {
            true => Ok(result),
            false =>  Err(CommandError {
                desc: "git clone failed",
                output: Some(result),
                detail: None,
            })
        }
    }

    /// Check if a directory exists and clone if it doesn't. This is currently
    /// very dumb in the sense that it only checks if a directory exists, not
    /// whether it's the git repo represented by `self` or even whether it's a
    /// git repository at all.
    ///
    /// This is the cli equivalent of doing: `test -d {local_path} || git clone
    /// --depth=1 --single-branch -b {self.branch} {remote_path} {local_path}`
    pub fn ensure_cloned(&self) -> Result<bool, CommandError> {
        if !directory_exists(&self.local_path) {
            return match self.clone() {
                Ok(_) => Ok(true),
                Err(e) => Err(e),
            }
        }

        Ok(false)
    }

    /// Fetch from the upstream. The same as doing `git fetch`.
    pub fn fetch(&self) -> Result<Output, CommandError> {
        if !directory_exists(&self.local_path) {
            return Err(CommandError {
                desc: "could not change to directory (repo not cloned?)",
                output: None,
                detail: None,
            })
        }
        let result = match Command::new("git")
            .current_dir(&self.local_path)
            .arg("fetch")
            .output() {
                Ok(r) => r,
                Err(e) => return Err(CommandError {
                    desc: "failed to execute process, see detail",
                    output: None,
                    detail: Some(format!("{}", e)),
                })
            };

        match result.status.success() {
            true => Ok(result),
            false =>  Err(CommandError {
                desc: "git fetch failed",
                output: Some(result),
                detail: None,
            })
        }
    }

    /// Check out a specific sha. The equivalent of doing `git checkout {sha}`.
    pub fn checkout(&self, sha: &str) -> Result<Output, CommandError> {
        let result = match Command::new("git")
            .current_dir(self.local_path)
            .arg("checkout")
            .arg(sha)
            .output() {
                Ok(r) => r,
                Err(e) => return Err(CommandError {
                    desc: "failed to execute process, see detail",
                    output: None,
                    detail: Some(format!("{}", e)),
                })
            };

        match result.status.success() {
            true => Ok(result),
            false =>  Err(CommandError {
                desc: "git checkout failed",
                output: Some(result),
                detail: None,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Git;
    use tempdir::TempDir;
    use ::verified_path::directory_exists;

    static KNOWN_SHA: &'static str = "529f5d02eb91bc9cf797a89049bd2286815455a4";

    #[test]
    fn test_git_clone() {
        let local_path = TempDir::new("deployer-git-test").unwrap().path().join("test_repo");
        let git = Git {
            branch: String::from("master"),
            remote_path: String::from("src/test/test_repo"),
            local_path: local_path.as_path(),
        };
        assert!(git.clone().is_ok());
        assert!(directory_exists(&local_path));
        assert!(git.clone().is_err());
    }

    #[test]
    fn test_git_ensure_cloned() {
        let local_path = TempDir::new("deployer-git-test").unwrap().path().join("test_repo");
        let git = Git {
            branch: String::from("master"),
            remote_path: String::from("src/test/test_repo"),
            local_path: local_path.as_path(),
        };

        let first_run = git.ensure_cloned();
        let second_run = git.ensure_cloned();
        assert!(first_run.is_ok());
        assert!(second_run.is_ok());
        match first_run {
            Ok(true) => (),
            _ => panic!("expected first run to have cloned")
        }
        match second_run {
            Ok(false) => (),
            _ => panic!("expected second run to not clone")
        }
    }

    #[test]
    fn test_git_fetch() {
        let local_path = TempDir::new("deployer-git-test").unwrap().path().join("test_repo");
        let git = Git {
            branch: String::from("master"),
            remote_path: String::from("src/test/test_repo"),
            local_path: local_path.as_path(),
        };

        assert!(git.clone().is_ok());
        assert!(git.ensure_cloned().is_ok());
        assert!(git.fetch().is_ok());
    }

    #[test]
    fn test_git_fetch_not_cloned() {
        let local_path = TempDir::new("deployer-git-test").unwrap().path().join("test_repo");
        let git = Git {
            branch: String::from("master"),
            remote_path: String::from("src/test/test_repo"),
            local_path: local_path.as_path(),
        };
        assert!(git.fetch().is_err());
    }

    #[test]
    fn test_git_checkout() {
        let local_path = TempDir::new("deployer-git-test").unwrap().path().join("test_repo");
        let git = Git {
            branch: String::from("master"),
            remote_path: String::from("src/test/test_repo"),
            local_path: local_path.as_path(),
        };

        assert!(git.ensure_cloned().is_ok());
        assert!(git.checkout(KNOWN_SHA).is_ok());
        assert!(git.checkout("not a real sha").is_err());
    }
}
