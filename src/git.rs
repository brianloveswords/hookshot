//! A library for interacting with the git cli.
//!
//! This library provides an interface for operating on git repository. It is
//! not intended to provide a 1-1 interface to the git cli but instead provide a
//! minimal interface to create the smallest checkout for a specific sha.

use error::CommandError;
use std::path::Path;
use std::process::{Command, Output};
use verified_path::directory_exists;
use message::RefType;

pub struct GitRepo {
    /// Owner of the repository
    pub owner: String,

    /// Name of the repository
    pub name: String,

    /// Reference to check out. We require this so we can create the smallest
    /// checkout possible.
    pub refstring: String,

    /// Reference type (whether this is a branch or a tag). Used to look up
    /// the appropriate action from the repository configuration.
    pub reftype: RefType,

    /// Specific SHA for this event.
    pub sha: String,

    /// Remote path to the repository. This can be a filesystem path if the
    /// `file://` protocol is used.
    pub remote_path: String,

    /// Local path of where to clone the repository.
    pub local_path: String,
}

pub trait ToGitRepo {
    fn to_git_repo(self, root: &str) -> GitRepo;
}

impl GitRepo {
    pub fn from<T: ToGitRepo>(other: T, root: &str) -> GitRepo {
        other.to_git_repo(root)
    }

    pub fn fully_qualified_branch(&self) -> String {
        format!("{}.{}.{}", &self.owner, &self.name, &self.refstring)
    }

    fn clone(&self) -> Result<Output, CommandError> {
        let output = Command::new("git")
                         .arg("clone")
                         .arg("--depth=1")
                         .arg("--single-branch")
                         .arg("-b")
                         .arg(&self.refstring)
                         .arg(&self.remote_path)
                         .arg(&self.local_path)
                         .output();

        let result = match output {
            Ok(r) => r,
            Err(e) => return Err(CommandError {
                desc: "failed to execute process, see detail",
                output: None,
                detail: Some(format!("{}", e)),
            }),
        };

        match result.status.success() {
            true => Ok(result),
            false => Err(CommandError {
                desc: "git clone failed",
                output: Some(result),
                detail: None,
            }),
        }
    }
    fn ensure_cloned(&self) -> Result<bool, CommandError> {
        if !directory_exists(&Path::new(&self.local_path)) {
            return match self.clone() {
                Ok(_) => Ok(true),
                Err(e) => Err(e),
            };
        }
        Ok(false)
    }

    fn fetch(&self) -> Result<Output, CommandError> {
        if let Err(e) = self.ensure_cloned() {
            return Err(e);
        }

        let output = Command::new("git")
                         .current_dir(&self.local_path)
                         .arg("fetch")
                         .arg("--tags")
                         .output();

        let result = match output {
            Ok(r) => r,
            Err(e) => return Err(CommandError {
                desc: "failed to execute process, see detail",
                output: None,
                detail: Some(format!("{}", e)),
            }),
        };

        match result.status.success() {
            true => Ok(result),
            false => Err(CommandError {
                desc: "git fetch failed",
                output: Some(result),
                detail: None,
            }),
        }
    }

    /// If a repo exists, fetch && reset it. If it doesn't, clone it
    ///
    /// This is currently very dumb in the sense that it only checks if
    /// a directory exists, not whether it's the git repo represented
    /// by `self` or even whether it's a git repository at all.
    ///
    /// This is the equivalent of doing:
    ///
    /// ```text
    /// (test -d <local_path> && \
    ///   cd <local_path> && \
    ///   git fetch && \
    ///   git reset --hard <ref>) || \
    /// git clone --depth=1 --single-branch -b <ref> <remote_path> <local_path>
    /// ```
    pub fn get_latest(&self) -> Result<Output, CommandError> {
        if let Err(e) = self.fetch() {
            return Err(e);
        }

        let output = Command::new("git")
                         .current_dir(&self.local_path)
                         .arg("reset")
                         .arg("--hard")
                         .arg(&self.sha)
                         .output();

        let result = match output {
            Ok(r) => r,
            Err(e) => return Err(CommandError {
                desc: "failed to execute process, see detail",
                output: None,
                detail: Some(format!("{}", e)),
            }),
        };

        match result.status.success() {
            true => Ok(result),
            false => Err(CommandError {
                desc: "git reset failed",
                output: Some(result),
                detail: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GitRepo;
    use message::RefType;
    use tempdir::TempDir;
    use verified_path::directory_exists;

    #[test]
    fn test_git_clone() {
        let local_path = TempDir::new("hookshot-git-test").unwrap().path().join("test_repo");
        let git = GitRepo {
            owner: String::from("test"),
            name: String::from("test"),
            refstring: String::from("master"),
            reftype: RefType::branch,
            sha: String::from("HEAD"),
            remote_path: String::from("src/test/test_repo"),
            local_path: String::from(local_path.to_str().unwrap()),
        };
        assert!(git.clone().is_ok());
        assert!(directory_exists(&local_path));
        assert!(git.clone().is_err());
    }

    #[test]
    fn test_git_ensure_cloned() {
        let local_path = TempDir::new("hookshot-git-test").unwrap().path().join("test_repo");
        let git = GitRepo {
            owner: String::from("test"),
            name: String::from("test"),
            refstring: String::from("master"),
            reftype: RefType::branch,
            sha: String::from("HEAD"),
            remote_path: String::from("src/test/test_repo"),
            local_path: String::from(local_path.to_str().unwrap()),
        };

        let first_run = git.ensure_cloned();
        let second_run = git.ensure_cloned();
        assert!(first_run.is_ok());
        assert!(second_run.is_ok());
        match first_run {
            Ok(true) => (),
            _ => panic!("expected first run to have cloned"),
        }
        match second_run {
            Ok(false) => (),
            _ => panic!("expected second run to not clone"),
        }
    }

    #[test]
    fn test_git_get_latest() {
        let local_path = TempDir::new("hookshot-git-test").unwrap().path().join("test_repo");
        let git = GitRepo {
            owner: String::from("test"),
            name: String::from("test"),
            refstring: String::from("master"),
            reftype: RefType::branch,
            sha: String::from("HEAD"),
            remote_path: String::from("src/test/test_repo"),
            local_path: String::from(local_path.to_str().unwrap()),
        };
        assert!(git.get_latest().is_ok());
    }

    #[test]
    fn test_git_fully_qualified_branch() {
        let git = GitRepo {
            owner: String::from("owner"),
            name: String::from("name"),
            refstring: String::from("branch"),
            reftype: RefType::branch,
            sha: String::from("HEAD"),
            remote_path: String::from("doesn't matter"),
            local_path: String::from("irrelevant"),
        };
        assert_eq!(git.fully_qualified_branch(), "owner.name.branch");
    }
}
