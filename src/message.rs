use git::{GitRepo, ToGitRepo};
use rustc_serialize::json::Json;
use rustc_serialize::json;

#[derive(Clone, Debug)]
pub struct GitHubMessage {
    branch: String,
    repo_name: String,
    owner: String,
    git_url: String,
    sha: String,
}

impl ToGitRepo for GitHubMessage {
    fn to_git_repo(self, root: &str) -> GitRepo {
        // Make the end part of the local_path. Do some very basic safety on the
        // string so it can't escape the container directory. This is intended
        // to prevent accidents, not malicious behavior -- that's what the
        // signature is (hopefully) for.
        let local_path_component = {
            let prefix = &self.owner;
            let path = format!("{}.{}.{}", prefix, self.repo_name, self.branch);
            path.replace("/", "!").replace("\\", "!")
        };

        GitRepo {
            owner: self.owner,
            name: self.repo_name,
            branch: self.branch,
            sha: self.sha,

            // TODO: fix this, use paths & path.join or something
            local_path: format!("{}/{}", root, local_path_component),
            remote_path: self.git_url,
        }
    }
}

impl GitHubMessage {
    pub fn from_str(json: &str) -> Result<GitHubMessage, &'static str> {

        let data = match Json::from_str(&json) {
            Ok(data) => data,
            Err(_) => return Err("could not parse json"),
        };

        let root_obj = data;

        let branch = {
            // "refs/heads/webhook-receiver"
            // "refs/tags/v1.0.0"
            let parts: Vec<_> = match root_obj.find("ref") {
                None => return Err("missing required field `ref`"),
                Some(v) => match v.as_string() {
                    None => return Err("could not read `ref` as string"),
                    Some(v) => v.split("/").collect(),
                },
            };
            match (parts.get(0), parts.get(1), parts.get(2)) {
                (Some(b), Some(reftype), Some(branch)) if *b == "refs" &&
                                                          (*reftype == "heads" ||
                                                           *reftype == "tags") => branch,
                _ => return Err("could not unpack `ref`"),
            }.to_string()
        };

        let repo_name = match root_obj.find_path(&["repository", "name"]) {
            Some(v) => match v.as_string() {
                Some(v) => v.to_string(),
                None => return Err("couldn't read `repository.name` as a string"),
            },
            None => return Err("missing `repository.name`"),
        };

        let sha = match root_obj.find_path(&["after"]) {
            Some(v) => match v.as_string() {
                Some(v) => v.to_string(),
                None => return Err("couldn't read `after` as a string"),
            },
            None => return Err("missing `after`"),
        };

        let owner = match root_obj.find_path(&["repository", "owner", "name"]) {
            Some(v) => match v.as_string() {
                Some(v) => v.to_string(),
                None => return Err("couldn't read `repository.owner.name` as a string"),
            },
            None => return Err("missing `repository.owner.name`"),
        };

        let git_url = match root_obj.find_path(&["repository", "ssh_url"]) {
            Some(v) => match v.as_string() {
                Some(v) => v.to_string(),
                None => return Err("couldn't read `repository.ssh_url` as a string"),
            },
            None => return Err("missing `repository.ssh_url`"),
        };

        Ok(GitHubMessage {
            branch: branch,
            repo_name: repo_name,
            owner: owner,
            sha: sha,
            git_url: git_url,
        })
    }
}

#[derive(RustcDecodable, Clone, Debug)]
pub struct SimpleMessage {
    /// The prefix to differentiate this deployment from another with
    /// possibly the same name.
    pub prefix: Option<String>,

    /// Branch to check out.
    pub branch: String,

    /// Remote path to the repository.
    pub remote: String,

    /// SHA to use. If unnecessary, just pass HEAD
    pub sha: String,

    /// Name of the repository. Used to construct the local path where
    /// the clone will be stored
    pub repository_name: String,
}

impl SimpleMessage {
    pub fn from_str(json: &str) -> Result<SimpleMessage, &'static str> {
        match json::decode::<SimpleMessage>(json) {
            Ok(msg) => Ok(msg),
            Err(_) => Err("could not decode json to message"),
        }
    }
}
impl ToGitRepo for SimpleMessage {
    fn to_git_repo(self, root: &str) -> GitRepo {
        let owner = self.prefix.unwrap_or("$".to_owned());

        // Make the end part of the local_path. Do some very basic safety on the
        // string so it can't escape the container directory. This is intended
        // to prevent accidents, not malicious behavior -- that's what the
        // signature is (hopefully) for.
        let local_path_component = {
            let prefix = owner.replace(".", "!");
            let path = format!("{}.{}.{}", prefix, self.repository_name, self.branch);
            path.replace("/", "!").replace("\\", "!")
        };

        GitRepo {
            // TODO: fix this, use paths or something
            name: self.repository_name,
            owner: owner,
            branch: self.branch,
            sha: self.sha,

            local_path: format!("{}/{}", root, local_path_component),
            remote_path: self.remote,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_message() {
        let json = r#"
        {
          "prefix": "brian",
          "repository_name": "stuff",
          "branch": "master",
          "remote": "the internet",
          "sha": "HEAD"
        }
        "#;

        let msg = match SimpleMessage::from_str(json) {
            Err(_) => panic!("expected to be able to decode message"),
            Ok(msg) => msg,
        };

        assert_eq!(msg.prefix, Some("brian".to_owned()));
        assert_eq!(msg.branch, "master");
        assert_eq!(msg.remote, "the internet");
        assert_eq!(msg.sha, "HEAD");
        assert_eq!(msg.repository_name, "stuff");
    }
}
