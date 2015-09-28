use toml;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::collections::BTreeMap;
use std::string::ToString;
use ::make_task::MakeTask;
use ::verified_path::VerifiedPath;
use ::error::Error;

// TODO: use https://crates.io/crates/url instead


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
pub struct BranchConfig<'a> {
    method: Option<DeployMethod>,
    task: Option<MakeTask<'a>>,
    playbook: Option<VerifiedPath>,
    inventory: Option<VerifiedPath>,
    notify_url: Option<URL>,
}
impl<'a> BranchConfig<'a> {
    pub fn task(&self) -> Option<&MakeTask<'a>> {
        match self.task {
            Some(ref t) => Some(&t),
            None => None,
        }
    }
}

pub type BranchConfigMap<'a> = BTreeMap<String, BranchConfig<'a>>;
pub type URL = String;

#[derive(Debug)]
pub struct RepoConfig<'a> {
    default_method: DeployMethod,
    default_task: Option<MakeTask<'a>>,
    default_playbook: Option<VerifiedPath>,
    default_notify_url: Option<URL>,
    branches: BranchConfigMap<'a>,
    project_root: &'a Path,
}

impl<'a> RepoConfig<'a> {
    pub fn lookup_branch(&self, name: &String) -> Option<&BranchConfig<'a>> {
        self.branches.get(name)
    }

    pub fn load(project_root: &'a Path) -> Result<RepoConfig<'a>, Error> {
        let config_path = project_root.join(".deployer.conf");
        let mut file = match File::open(&config_path) {
            Ok(file) => file,
            Err(_) => return Err(Error {
                desc: "could not open deployer configuration",
                subject: Some(String::from(config_path.to_str().unwrap())),
            }),
        };
        let mut contents = String::new();
        if file.read_to_string(&mut contents).is_err() {
            return Err(Error {
                desc: "could not read file contents",
                subject: Some(String::from(config_path.to_str().unwrap())),
            })
        };
        Self::from_str(&contents, project_root)
    }

    pub fn from_str(string: &str, project_root: &'a Path) -> Result<RepoConfig<'a>, Error> {
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
            LookupResult::Value(v) => match MakeTask::new(project_root, v) {
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
            LookupResult::Value(v) =>
                match VerifiedPath::file(Some(project_root), Path::new(v)) {
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
                    LookupResult::Value(v) => match MakeTask::new(project_root, v) {
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
                    LookupResult::Value(v) =>
                        match VerifiedPath::file(Some(project_root), Path::new(v)) {
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
                    LookupResult::Value(v) =>
                        match VerifiedPath::file(Some(project_root), Path::new(v)) {
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
            project_root: project_root,
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

#[cfg(test)]
mod tests {
    use super::{RepoConfig};
    use std::path::Path;

    #[test]
    fn test_valid_configuration() {
        // let toml = r#"
        //     [defaults]
        //     method = "ansible"
        //     task = "deploy"
        //     playbook = "ansible/deploy.yml"

        //     [branches.production]
        //     playbook = "ansible/production.yml"
        //     inventory = "ansible/inventory/production"

        //     [branches.staging]
        //     inventory = "ansible/inventory/staging"
        //     notify_url = "http://example.org"

        //     [branches.brian-test-branch]
        //     method = "makefile"
        //     task = "self-deploy"
        // "#;

        let project_root = Path::new("./src/test/repo_config");
        let config = RepoConfig::load(project_root).unwrap();
        println!("{:?}", config);

        assert_eq!(config.default_method.to_string(), "ansible");
        assert!(config.default_task.is_some());
        assert_eq!(config.default_task.unwrap().to_string(), "deploy");
        assert!(config.default_playbook.is_some());
        assert_eq!(config.default_playbook.unwrap().path(), "ansible/deploy.yml");
        assert!(config.default_notify_url.is_none());

        // production config
        {
            let config = config.branches.get("production").unwrap();
            let playbook = config.playbook.clone().unwrap().path();
            let inventory = config.inventory.clone().unwrap().path();
            assert_eq!(playbook, "ansible/production.yml");
            assert_eq!(inventory, "ansible/inventory/production");
            assert!(config.method.is_none());
            assert!(config.task.is_none());
            assert!(config.notify_url.is_none());
        }
        // staging config
        {
            let config = config.branches.get("staging").unwrap();
            let inventory = config.inventory.clone().unwrap().path();
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
