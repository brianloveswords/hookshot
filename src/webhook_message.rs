use regex::{Regex, Captures};
use rustc_serialize::json::Json;
use std::any::Any;
use std::thread;

#[derive(Clone, Debug)]
pub struct Message {
    branch: Option<String>,
    tag: Option<String>,
    head_sha: String,
    repo_name: String,
    owner: String,
    git_url: String,
}

#[derive(Debug, Clone, Copy)]
pub enum ErrorCode {
    InvalidRef,
    InvalidSha,
    InvalidRepoName,
    InvalidRepoOwner,
    InvalidGitUrl,
    MissingRef,
    MissingSha,
    MissingRepoName,
    MissingRepoOwner,
    MissingGitUrl,
    InvalidJson,
    Unknown,
}

#[derive(Debug)]
pub struct Error {
    code: ErrorCode,
    detail: &'static str,
    original_error: Option<Box<Any + Send + 'static>>,
}

pub type ParseResult = Result<Message, Error>;

impl Message {
    pub fn from_string(json_string: String) -> ParseResult {
        let result = thread::spawn(move || {

            let data = match Json::from_str(&json_string) {
                Ok(data) => data,
                Err(e) => return Err(Error {
                    code: ErrorCode::InvalidJson,
                    detail: "could not parse json",
                    original_error: Some(Box::new(e)),
                }),
            };

            let root_obj = data;

            let (branch, tag) = {
                let tag_re = Regex::new(r"^refs/tags/(.*)$").unwrap();
                let branch_re = Regex::new(r"^refs/heads/(.*)$").unwrap();

                let ref_val = match root_obj.find("ref") {
                    Some(v) => match v.as_string() {
                        Some(v) => v,
                        None => return Err(Error {
                            code: ErrorCode::InvalidRef,
                            detail: "ref must be a string",
                            original_error: None,
                        }),
                    },
                    None => return Err(Error {
                        code: ErrorCode::MissingRef,
                        detail: "ref must exist",
                        original_error: None,
                    })

                };

                let branch_caps = branch_re.captures(ref_val);
                let tag_caps = tag_re.captures(ref_val);

                if branch_caps.is_none() && tag_caps.is_none() {
                    return Err(Error {
                        code: ErrorCode::InvalidRef,
                        detail: "missing both branch & tag (bad ref?)",
                        original_error: None,
                    });
                }

                let tag = tag_caps.map(first_capture_to_string);
                let branch = branch_caps.map(first_capture_to_string);


                (branch, tag)
            };

            // TODO: turn these into macros, this is a ton of boilerplate
            let head_sha = match root_obj.find("after") {
                Some(v) => match v.as_string() {
                    Some(v) => v.to_string(),
                    None => return Err(Error {
                        code: ErrorCode::InvalidSha,
                        detail: "'after' must be a string",
                        original_error: None,
                    })
                },
                None => return Err(Error {
                    code: ErrorCode::MissingSha,
                    detail: "'after' must be present",
                    original_error: None,
                }),
            };


            let repo_name = match root_obj.find_path(&["repository", "name"]) {
                Some(v) => match v.as_string() {
                    Some(v) => v.to_string(),
                    None => return Err(Error {
                        code: ErrorCode::InvalidRepoName,
                        detail: "'repository.name' must be a string",
                        original_error: None,
                    })
                },
                None => return Err(Error {
                    code: ErrorCode::MissingRepoName,
                    detail: "'repository.name' must be present",
                    original_error: None,
                }),
            };

            let owner = match root_obj.find_path(&["repository", "owner", "name"]) {
                Some(v) => match v.as_string() {
                    Some(v) => v.to_string(),
                    None => return Err(Error {
                        code: ErrorCode::InvalidRepoOwner,
                        detail: "'repository.owner.name' must be a string",
                        original_error: None,
                    })
                },
                None => return Err(Error {
                    code: ErrorCode::MissingRepoOwner,
                    detail: "'repository.owner.name' must be present",
                    original_error: None,
                }),
            };

            let git_url = match root_obj.find_path(&["repository", "git_url"]) {
                Some(v) => match v.as_string() {
                    Some(v) => v.to_string(),
                    None => return Err(Error {
                        code: ErrorCode::InvalidGitUrl,
                        detail: "'repository.git_url' must be a string",
                        original_error: None,
                    })
                },
                None => return Err(Error {
                    code: ErrorCode::MissingGitUrl,
                    detail: "'repository.git_url' must be present",
                    original_error: None,
                }),
            };

            Ok(Message {
                branch: branch,
                tag: tag,
                head_sha: head_sha,
                repo_name: repo_name,
                owner: owner,
                git_url: git_url,
            })
        }).join();

        // Nuclear error handling
        match result {
            Ok(msg) => msg,
            Err(err) => Err(Error {
                code: ErrorCode::Unknown,
                detail: "failed to construct msg",
                original_error: Some(err),
            }),
        }
    }
}

fn first_capture_to_string(c: Captures) -> String {
    c.at(1).unwrap().to_string()
}

#[cfg(test)]
mod tests {
    use super::{Message, ErrorCode, Error};

    #[test]
    fn test_constructor() {
        let raw_message = r#"{
          "ref": "refs/heads/test-branch",
          "after": "452e649d25993d81f107649689916df749bb3e27",
          "repository": {
            "name": "test-repo",
            "git_url": "git://github.com/test-owner/test-repo.git",
            "owner": {
              "name": "test-owner"
            }
          }
        }"#;

        let msg = Message::from_string(raw_message.to_string()).unwrap();
        assert_eq!(msg.git_url, "git://github.com/test-owner/test-repo.git");
        assert_eq!(msg.branch, Some("test-branch".to_string()));
        assert_eq!(msg.tag, None);
        assert_eq!(msg.owner, "test-owner");
        assert_eq!(msg.head_sha, "452e649d25993d81f107649689916df749bb3e27");
    }

    #[test]
    fn test_bad_json() {
        let raw_message = r"this is not good json :(";
        match Message::from_string(raw_message.to_string()) {
            Err(Error{ code: ErrorCode::InvalidJson, .. }) => assert!(true),
            _ => panic!("wrong error code"),
        }
    }

    #[test]
    fn test_bad_ref() {
        let raw_message = r#"{
          "ref": "this is a terrible ref",
          "after": "452e649d25993d81f107649689916df749bb3e27",
          "repository": {
            "name": "test-repo",
            "git_url": "git://github.com/test-owner/test-repo.git",
            "owner": {
              "name": "test-owner"
            }
          }
        }"#;

        let msg = Message::from_string(raw_message.to_string());
        match msg {
            Err(Error{ code: ErrorCode::InvalidRef, .. }) => assert!(true),
            _ => panic!("wrong error code"),
        }
    }

    #[test]
    fn test_missing_ref() {
        let raw_message = r#"{
          "after": "452e649d25993d81f107649689916df749bb3e27",
          "repository": {
            "name": "test-repo",
            "git_url": "git://github.com/test-owner/test-repo.git",
            "owner": {
              "name": "test-owner"
            }
          }
        }"#;

        let msg = Message::from_string(raw_message.to_string());
        match msg {
            Err(Error{ code: ErrorCode::MissingRef, .. }) => assert!(true),
            _ =>  {
                println!("{:?}", msg);
                panic!("wrong error code");
            },
        }
    }

    #[test]
    fn test_missing_sha() {
        let raw_message = r#"{
          "ref": "refs/heads/test-branch",
          "repository": {
            "name": "test-repo",
            "git_url": "git://github.com/test-owner/test-repo.git",
            "owner": {
              "name": "test-owner"
            }
          }
        }"#;

        let msg = Message::from_string(raw_message.to_string());
        match msg {
            Err(Error{ code: ErrorCode::MissingSha, .. }) => assert!(true),
            _ =>  {
                println!("{:?}", msg);
                panic!("wrong error code");
            },
        }
    }

    #[test]
    fn test_bad_sha() {
        let raw_message = r#"{
          "after": [],
          "ref": "refs/heads/test-branch",
          "repository": {
            "name": "test-repo",
            "git_url": "git://github.com/test-owner/test-repo.git",
            "owner": {
              "name": "test-owner"
            }
          }
        }"#;

        let msg = Message::from_string(raw_message.to_string());
        match msg {
            Err(Error{ code: ErrorCode::InvalidSha, .. }) => assert!(true),
            _ =>  {
                println!("{:?}", msg);
                panic!("wrong error code");
            },
        }
    }

    #[test]
    fn test_missing_repo_name() {
        let raw_message = r#"{
          "after": "abc",
          "ref": "refs/heads/test-branch",
          "repository": {
            "git_url": "git://github.com/test-owner/test-repo.git",
            "owner": {
              "name": "test-owner"
            }
          }
        }"#;

        let msg = Message::from_string(raw_message.to_string());
        match msg {
            Err(Error{ code: ErrorCode::MissingRepoName, .. }) => assert!(true),
            _ =>  {
                println!("{:?}", msg);
                panic!("wrong error code");
            },
        }
    }

    #[test]
    fn test_invalid_repo_name() {
        let raw_message = r#"{
          "after": "abc",
          "ref": "refs/heads/test-branch",
          "repository": {
            "name" : [],
            "git_url": "git://github.com/test-owner/test-repo.git",
            "owner": {
              "name": "test-owner"
            }
          }
        }"#;

        let msg = Message::from_string(raw_message.to_string());
        match msg {
            Err(Error{ code: ErrorCode::InvalidRepoName, .. }) => assert!(true),
            _ =>  {
                println!("{:?}", msg);
                panic!("wrong error code");
            },
        }
    }

    #[test]
    fn test_missing_repo_owner() {
        let raw_message = r#"{
          "after": "abc",
          "ref": "refs/heads/test-branch",
          "repository": {
            "name": "whatever",
            "git_url": "git://github.com/test-owner/test-repo.git",
            "owner": {
            }
          }
        }"#;

        let msg = Message::from_string(raw_message.to_string());
        match msg {
            Err(Error{ code: ErrorCode::MissingRepoOwner, .. }) => assert!(true),
            _ =>  {
                println!("{:?}", msg);
                panic!("wrong error code");
            },
        }
    }

    #[test]
    fn test_invalid_repo_owner() {
        let raw_message = r#"{
          "after": "abc",
          "ref": "refs/heads/test-branch",
          "repository": {
            "name": "whatever",
            "git_url": "git://github.com/test-owner/test-repo.git",
            "owner": {
              "name": []
            }
          }
        }"#;

        let msg = Message::from_string(raw_message.to_string());
        match msg {
            Err(Error{ code: ErrorCode::InvalidRepoOwner, .. }) => assert!(true),
            _ =>  {
                println!("{:?}", msg);
                panic!("wrong error code");
            },
        }
    }

    #[test]
    fn test_missing_git_url() {
        let raw_message = r#"{
          "after": "abc",
          "ref": "refs/heads/test-branch",
          "repository": {
            "name": "whatever",
            "owner": {
              "name": "test-owner"
            }
          }
        }"#;

        let msg = Message::from_string(raw_message.to_string());
        match msg {
            Err(Error{ code: ErrorCode::MissingGitUrl, .. }) => assert!(true),
            _ =>  {
                println!("{:?}", msg);
                panic!("wrong error code");
            },
        }
    }

    #[test]
    fn test_invalid_git_url() {
        let raw_message = r#"{
          "after": "abc",
          "ref": "refs/heads/test-branch",
          "repository": {
            "name": "whatever",
            "git_url": [],
            "owner": {
              "name": "test-owner"
            }
          }
        }"#;

        let msg = Message::from_string(raw_message.to_string());
        match msg {
            Err(Error{ code: ErrorCode::InvalidGitUrl, .. }) => assert!(true),
            _ =>  {
                println!("{:?}", msg);
                panic!("wrong error code");
            },
        }
    }

}
