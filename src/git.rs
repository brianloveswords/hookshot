use std::process::{Command, Output};
use std::path::Path;
use ::verified_path::directory_exists;

use ::error::CommandError;

pub struct Git<'a> {
    owner: &'a str,
    repo_name: &'a str,
    origin: &'a str,
    checkout_root: &'a Path,
}
impl<'a> Git<'a> {
    pub fn clone(&self) -> Result<Output, CommandError> {
        let local_path = self.checkout_root.join(self.prefixed_repo_name());
        let result = match Command::new("git")
            .arg("clone")
            .arg(self.origin)
            .arg(local_path)
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

    pub fn ensure_cloned(&self) -> Result<bool, CommandError> {
        let local_path = self.checkout_root.join(self.prefixed_repo_name());
        if !directory_exists(&local_path) {
            return match self.clone() {
                Ok(_) => Ok(true),
                Err(e) => Err(e),
            }
        }

        Ok(false)
    }

    pub fn fetch(&self) -> Result<Output, CommandError> {
        let local_path = self.checkout_root.join(self.prefixed_repo_name());
        if !directory_exists(&local_path) {
            return Err(CommandError {
                desc: "could not change to directory (repo not cloned?)",
                output: None,
                detail: None,
            })
        }
        let result = match Command::new("git")
            .current_dir(local_path)
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

    pub fn checkout(&self, commitish: &str) -> Result<Output, CommandError> {
        let local_path = self.checkout_root.join(self.prefixed_repo_name());
        let result = match Command::new("git")
            .current_dir(local_path)
            .arg("checkout")
            .arg(commitish)
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

    fn prefixed_repo_name(&self) -> String {
        format!("{}.{}", self.owner, self.repo_name)
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
        let tmpdir = TempDir::new("deployer-git-test").unwrap();
        let git = Git {
            owner: "brian",
            repo_name: "creamsickle",
            origin: "src/test/test_repo",
            checkout_root: tmpdir.path(),
        };
        let checkout_path = git.checkout_root.join(git.prefixed_repo_name());
        assert!(git.clone().is_ok());
        assert!(directory_exists(&checkout_path));
        assert!(git.clone().is_err());
    }

    #[test]
    fn test_git_ensure_cloned() {
        let tmpdir = TempDir::new("deployer-git-test").unwrap();
        let git = Git {
            owner: "brian",
            repo_name: "creamsickle",
            origin: "src/test/test_repo",
            checkout_root: tmpdir.path(),
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
        let tmpdir = TempDir::new("deployer-git-test").unwrap();
        let git = Git {
            owner: "brian",
            repo_name: "creamsickle",
            origin: "src/test/test_repo",
            checkout_root: tmpdir.path(),
        };

        assert!(git.clone().is_ok());
        assert!(git.ensure_cloned().is_ok());
        assert!(git.fetch().is_ok());
    }

    #[test]
    fn test_git_fetch_not_cloned() {
        let tmpdir = TempDir::new("deployer-git-test").unwrap();
        let git = Git {
            owner: "brian",
            repo_name: "creamsickle",
            origin: "src/test/test_repo",
            checkout_root: tmpdir.path(),
        };
        assert!(git.fetch().is_err());
    }

    #[test]
    fn test_git_checkout() {
        let tmpdir = TempDir::new("deployer-git-test").unwrap();
        let git = Git {
            owner: "brian",
            repo_name: "creamsickle",
            origin: "src/test/test_repo",
            checkout_root: tmpdir.path(),
        };

        assert!(git.ensure_cloned().is_ok());
        assert!(git.checkout(KNOWN_SHA).is_ok());
        assert!(git.checkout("not a real sha").is_err());
    }
}
