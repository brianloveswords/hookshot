use toml;
use std::path::Path;
use std::io::Read;
use std::fs::{self, File};
use std::collections::BTreeMap;
use std::string::ToString;

pub type URL = String;
pub type BranchConfigMap = BTreeMap<String, BranchConfig>;

#[derive(Debug)]
pub struct Error {
    desc: &'static str,
    subject: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VerifiedPath {
    path: String,
}
impl VerifiedPath {
    fn new(path: &str) -> Result<VerifiedPath, Error> {
        match file_exists(Path::new(&path)) {
            true => Ok(VerifiedPath { path: path.to_string() }),
            false => Err(Error {
                desc: "file doesn't exist",
                subject: Some(path.to_string()),
            }),
        }
    }
}
impl ToString for VerifiedPath {
    fn to_string(&self) -> String { self.path.clone()  }
}

#[derive(Debug, Clone)]
pub struct MakeTask {
    task: String,
}
impl MakeTask {
    fn new(task: &str) -> Result<MakeTask, Error> {

        let makefile_contents = {
            let mut f = match File::open("Makefile") {
                Ok(f) => f,
                Err(_) => return Err(Error {
                    desc: "can't open Makefile",
                    subject: None,
                }),
            };

            let mut contents = String::new();
            f.read_to_string(&mut contents).unwrap();
            contents
        };

        let mut task_header = task.to_string();
        task_header.push(':');

        let has_task = makefile_contents.lines_any()
            .any(|line| line.starts_with(&task_header));

        match has_task {
            true => Ok(MakeTask { task: task.to_string() }),
            false => Err(Error {
                desc: "Makefile does not have specified task",
                subject: Some(task.to_string()),
            }),
        }
    }

}
impl ToString for MakeTask {
    fn to_string(&self) -> String { self.task.clone() }
}

#[derive(Debug)]
pub struct BranchConfig {
    method: Option<DeployMethod>,
    task: Option<MakeTask>,
    playbook: Option<VerifiedPath>,
    inventory: Option<VerifiedPath>,
    notify_url: Option<URL>,
}

#[derive(Debug, Clone)]
pub enum DeployMethod {
    Ansible,
    Makefile,
}
impl ToString for DeployMethod {
    fn to_string(&self) -> String {
        match *self {
            DeployMethod::Ansible => String::from("ansible"),
            DeployMethod::Makefile => String::from("makefile"),
        }
    }
}

#[derive(Debug)]
pub struct RepoConfig {
    default_method: DeployMethod,
    default_task: Option<MakeTask>,
    default_playbook: Option<VerifiedPath>,
    // TODO: create VerifiedUrl, make sure this looks like a URL.
    default_notify_url: Option<URL>,
    branches: BranchConfigMap,
}

impl RepoConfig {
    pub fn from_str(string: &str) -> Result<RepoConfig, Error> {
        let root = match toml::Parser::new(string).parse() {
            Some(value) => value,
            None => return Err(Error {
                desc: "could not parse toml",
                subject: None,
            }),
        };

        let defaults = match root.get("defaults") {
            Some(value) => value,
            None => return Err(Error {
                desc: "missing 'defaults' section",
                subject: Some(String::from("defaults")),
            }),
        };

        let default_method = match lookup_as_string(defaults, "method") {
            LookupResult::Missing => DeployMethod::Ansible,
            LookupResult::WrongType => return Err(Error {
                desc: "could not read 'defaults.method' as string",
                subject: Some(String::from("defaults.method")),
            }),
            LookupResult::Value(v) => match v {
                "ansible" => DeployMethod::Ansible,
                "makefile" | "make" => DeployMethod::Makefile,
                _ => return Err(Error {
                    desc: "invalid type, valid values are 'ansible' and 'makefile'",
                    subject: Some(String::from("defaults.method")),
                }),
            }
        };

        let default_task = match lookup_as_string(defaults, "task") {
            LookupResult::Missing => None,
            LookupResult::WrongType => return Err(Error {
                desc: "could not read 'defaults.task' as string",
                subject: Some(String::from("defaults.task")),
            }),
            LookupResult::Value(v) => match MakeTask::new(v) {
                Ok(v) => Some(v),
                Err(err) => return Err(err),
            }
        };

        let default_playbook = match lookup_as_string(defaults, "playbook") {
            LookupResult::Missing => None,
            LookupResult::WrongType => return Err(Error {
                desc: "could not read 'defaults.playbook' as string",
                subject: Some(String::from("defaults.playbook")),
            }),
            LookupResult::Value(v) => match VerifiedPath::new(v) {
                Ok(v) => Some(v),
                Err(err) => return Err(err),
            },
        };

        let default_notify_url = match lookup_as_string(defaults, "notify_url") {
            LookupResult::Missing => None,
            LookupResult::WrongType => return Err(Error {
                desc: "could not read 'defaults.notify_url' as string",
                subject: Some(String::from("defaults.notify_url")),
            }),
            LookupResult::Value(v) => Some(v.to_string()),
        };

        let raw_branches = match root.get("branches") {
            None => return Err(Error{
                desc: "must configure at least one branch (missing [branches.*])",
                subject: Some(String::from("branches.*")),
            }),
            Some(v) => match v.as_table() {
                None => return Err(Error {
                    desc: "'branches' must be a table",
                    subject: Some(String::from("branches")),
                }),
                Some(v) => v
            }
        };

        let mut branches = BranchConfigMap::new();

        for (key, table) in raw_branches.iter() {
            if table.as_table().is_none() {
                return Err(Error {
                    desc: "every 'branches' must be a table",
                    subject: Some(key.clone()),
                });
            }

            branches.insert(key.clone(), BranchConfig {
                task: match lookup_as_string(table, "task") {
                    LookupResult::Missing => None,
                    LookupResult::WrongType => return Err(Error {
                        desc: "branch 'task' not a string",
                        subject: Some(format!("branch.{}.task", key)),
                    }),
                    LookupResult::Value(v) => match MakeTask::new(v) {
                        Ok(v) => Some(v),
                        Err(err) => return Err(err),
                    }
                },
                method: match lookup_as_string(table, "method") {
                    LookupResult::Missing => None,
                    LookupResult::WrongType => return Err(Error {
                        desc: "branch 'type' not a string",
                        subject: Some(format!("branch.{}.method", key)),
                    }),
                    LookupResult::Value(v) => match v {
                        "ansible" => Some(DeployMethod::Ansible),
                        "makefile" | "make" => Some(DeployMethod::Makefile),
                        _ => return Err(Error {
                            desc: "invalid 'type', valid values are 'ansible' and 'makefile'",
                            subject: Some(format!("branch.{}.method", key)),
                        }),
                    }
                },
                playbook: match lookup_as_string(table, "playbook") {
                    LookupResult::Missing => None,
                    LookupResult::WrongType => return Err(Error {
                        desc: "branch 'playbook' not a string",
                        subject: Some(format!("branch.{}.playbook", key)),
                    }),
                    LookupResult::Value(v) => match VerifiedPath::new(v) {
                        Ok(v) => Some(v),
                        Err(err) => return Err(err),
                    },
                },
                inventory: match lookup_as_string(table, "inventory") {
                    LookupResult::Missing => None,
                    LookupResult::WrongType => return Err(Error {
                        desc: "branch 'inventory' not a string",
                        subject: Some(format!("branch.{}.inventory", key)),
                    }),
                    LookupResult::Value(v) => match VerifiedPath::new(v) {
                        Ok(v) => Some(v),
                        Err(err) => return Err(err),
                    },
                },
                notify_url: match lookup_as_string(table, "notify_url") {
                    LookupResult::Missing => None,
                    LookupResult::WrongType => return Err(Error {
                        desc: "branch 'notify_url' not a string",
                        subject: Some(format!("branch.{}.notify_url", key)),
                    }),
                    LookupResult::Value(v) => Some(v.to_string()),
                },
            });
        }

        Ok(RepoConfig {
            default_method: default_method,
            default_task: default_task,
            default_playbook: default_playbook,
            default_notify_url: default_notify_url,
            branches: branches,
        })
    }
}

enum LookupResult<'a> {
    Missing,
    WrongType,
    Value(&'a str),
}

fn lookup_as_string<'a>(obj: &'a toml::Value, key: &'static str) -> LookupResult<'a> {
    match obj.lookup(key) {
        None => LookupResult::Missing,
        Some(v) => {
            match v.as_str() {
                None => LookupResult::WrongType,
                Some(v) => LookupResult::Value(v),
            }
        }
    }
}

fn file_exists(full_path: &Path) -> bool {
    match fs::metadata(full_path) {
        Err(_) => false,
        Ok(f) => f.is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::{RepoConfig};
    use std::env;
    use std::path::Path;

    fn reset_test_dir() {
        assert!(env::set_current_dir(Path::new("./src/test")).is_ok());
    }

    #[test]
    fn test_valid_configuration() {
        reset_test_dir();

        let toml = r#"
            [defaults]
            method = "ansible"
            task = "deploy"
            playbook = "ansible/deploy.yml"

            [branches.production]
            playbook = "ansible/production.yml"
            inventory = "ansible/inventory/production"

            [branches.staging]
            inventory = "ansible/inventory/staging"
            notify_url = "http://example.org"

            [branches.brian-test-branch]
            method = "makefile"
            task = "self-deploy"
        "#;

        let config = RepoConfig::from_str(toml).unwrap();
        println!("{:?}", config);

        assert_eq!(config.default_method.to_string(), "ansible");
        assert!(config.default_task.is_some());
        assert_eq!(config.default_task.unwrap().to_string(), "deploy");
        assert!(config.default_playbook.is_some());
        assert_eq!(config.default_playbook.unwrap().to_string(), "ansible/deploy.yml");
        assert!(config.default_notify_url.is_none());

        // production config
        {
            let config = config.branches.get("production").unwrap();
            let playbook = config.playbook.clone().unwrap().to_string();
            let inventory = config.inventory.clone().unwrap().to_string();
            assert_eq!(playbook, "ansible/production.yml");
            assert_eq!(inventory, "ansible/inventory/production");
            assert!(config.method.is_none());
            assert!(config.task.is_none());
            assert!(config.notify_url.is_none());
        }
        // staging config
        {
            let config = config.branches.get("staging").unwrap();
            let inventory = config.inventory.clone().unwrap().to_string();
            let notify_url = config.notify_url.clone().unwrap();
            assert_eq!(inventory, "ansible/inventory/staging");
            assert!(config.playbook.is_none());
            assert!(config.method.is_none());
            assert!(config.task.is_none());
            assert_eq!(notify_url, "http://example.org");
        }
        // brian-test-branch config
        {
            let config = config.branches.get("brian-test-branch").unwrap();
            let method = config.method.clone().unwrap();

            assert_eq!(method.to_string(), "makefile");
            assert!(config.playbook.is_none());
            assert!(config.inventory.is_none());
            assert!(config.notify_url.is_none());
        }

    }
}
