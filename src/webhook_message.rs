use std::any::Any;
use regex::{Regex, Captures};
use rustc_serialize::json::Json;
use rustc_serialize::json;
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
                        code: ErrorCode::InvalidRef,
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


            Ok(Message {
                branch: branch,
                tag: tag,
                head_sha: root_obj.find("after")
                    .unwrap().as_string().unwrap().to_string(),
                repo_name: root_obj.find_path(&["repository", "name"])
                    .unwrap().as_string().unwrap().to_string(),
                owner: root_obj.find_path(&["repository", "owner", "name"])
                    .unwrap().as_string().unwrap().to_string(),
                git_url: root_obj.find_path(&["repository", "git_url"])
                    .unwrap().as_string().unwrap().to_string(),
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

        match Message::from_string(raw_message.to_string()) {
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

        match Message::from_string(raw_message.to_string()) {
            Err(Error{ code: ErrorCode::InvalidRef, .. }) => assert!(true),
            _ => panic!("wrong error code"),
        }
    }


}
