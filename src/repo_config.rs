use toml;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::collections::BTreeMap;
use std::string::ToString;
use ::make_task::MakeTask;
use ::ansible_task::AnsibleTask;
use ::verified_path::VerifiedPath;
use ::error::Error;

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
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
    pub method: DeployMethod,
    make_task: Option<MakeTask<'a>>,
    ansible_task: Option<AnsibleTask<'a>>,
    notify_url: Option<URL>,
}
impl<'a> BranchConfig<'a> {
    pub fn make_task(&self) -> Option<&MakeTask<'a>> {
        match self.make_task {
            Some(ref t) => Some(t),
            None => None,
        }
    }
    pub fn ansible_task(&self) -> Option<&AnsibleTask<'a>> {
        match self.ansible_task {
            Some(ref t) => Some(t),
            None => None,
        }
    }
}

pub type BranchConfigMap<'a> = BTreeMap<String, BranchConfig<'a>>;

// TODO: use https://crates.io/crates/url instead
pub type URL = String;

#[derive(Debug)]
pub struct RepoConfig<'a> {
    default_method: DeployMethod,
    default_task: Option<MakeTask<'a>>,
    default_playbook: Option<VerifiedPath>,
    default_inventory: Option<VerifiedPath>,
    default_notify_url: Option<URL>,
    branch: BranchConfigMap<'a>,
    project_root: &'a Path,
}

impl<'a> RepoConfig<'a> {
    pub fn lookup_branch(&self, name: &String) -> Option<&BranchConfig<'a>> {
        self.branch.get(name)
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

        let default = match root.get("default") {
            Some(value) => value,
            None => return Err(Error {
                desc: "missing 'default' section",
                subject: Some(String::from("default")),
            }),
        };

        let default_method = match lookup_as_string(default, "method") {
            LookupResult::Missing => DeployMethod::Makefile,
            LookupResult::WrongType => return Err(Error {
                desc: "could not read 'default.method' as string",
                subject: Some(String::from("default.method")),
            }),
            LookupResult::Value(v) => match v {
                "ansible" => DeployMethod::Ansible,
                "makefile" | "make" => DeployMethod::Makefile,
                _ => return Err(Error {
                    desc: "invalid type, valid values are 'ansible' and 'makefile'",
                    subject: Some(String::from("default.method")),
                }),
            }
        };

        let default_task = match lookup_as_string(default, "task") {
            LookupResult::Missing => None,
            LookupResult::WrongType => return Err(Error {
                desc: "could not read 'default.task' as string",
                subject: Some(String::from("default.task")),
            }),
            LookupResult::Value(v) => match MakeTask::new(project_root, v) {
                Ok(v) => Some(v),
                Err(err) => return Err(err),
            }
        };

        let default_playbook = match lookup_as_string(default, "playbook") {
            LookupResult::Missing => None,
            LookupResult::WrongType => return Err(Error {
                desc: "could not read 'default.playbook' as string",
                subject: Some(String::from("default.playbook")),
            }),
            LookupResult::Value(v) =>
                match VerifiedPath::file(Some(project_root), Path::new(v)) {
                    Ok(v) => Some(v),
                    Err(err) => return Err(err),
                },
        };

        let default_inventory = match lookup_as_string(default, "inventory") {
            LookupResult::Missing => None,
            LookupResult::WrongType => return Err(Error {
                desc: "could not read 'default.inventory' as string",
                subject: Some(String::from("default.inventory")),
            }),
            LookupResult::Value(v) =>
                match VerifiedPath::file(Some(project_root), Path::new(v)) {
                    Ok(v) => Some(v),
                    Err(err) => return Err(err),
                },
        };

        let default_notify_url = match lookup_as_string(default, "notify_url") {
            LookupResult::Missing => None,
            LookupResult::WrongType => return Err(Error {
                desc: "could not read 'default.notify_url' as string",
                subject: Some(String::from("default.notify_url")),
            }),
            LookupResult::Value(v) => Some(v.to_string()),
        };

        let raw_branch = match root.get("branch") {
            None => return Err(Error{
                desc: "must configure at least one branch (missing [branch.*])",
                subject: Some(String::from("branch.*")),
            }),
            Some(v) => match v.as_table() {
                None => return Err(Error {
                    desc: "'branch' must be a table",
                    subject: Some(String::from("branch")),
                }),
                Some(v) => v
            }
        };

        let mut branch = BranchConfigMap::new();

        for (key, table) in raw_branch.iter() {
            if table.as_table().is_none() {
                return Err(Error {
                    desc: "every 'branch' must be a table",
                    subject: Some(key.clone()),
                });
            }

            let method = match lookup_as_string(table, "method") {
                LookupResult::Missing => default_method,
                LookupResult::WrongType => return Err(Error {
                    desc: "could not read 'default.method' as string",
                    subject: Some(String::from("default.method")),
                }),
                LookupResult::Value(v) => match v {
                    "ansible" => DeployMethod::Ansible,
                    "makefile" | "make" => DeployMethod::Makefile,
                    _ => return Err(Error {
                        desc: "invalid type, valid values are 'ansible' and 'makefile'",
                        subject: Some(String::from("default.method")),
                    }),
                }
            };

            let playbook = match lookup_as_string(table, "playbook") {
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
            };
            let inventory = match lookup_as_string(table, "inventory") {
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
            };

            let ansible_task = if method == DeployMethod::Ansible {
                // This complicated looking match tries to create a
                // playbook/inventory combination by first preferring
                // configuration for the specific branch and falling back to
                // default where necessary.
                match (playbook, inventory, default_playbook.clone(), default_inventory.clone()) {
                    (Some(p), Some(i), _, _) |
                    (None, Some(i), Some(p), _) |
                    (Some(p), None, None, Some(i)) |
                    (None, None, Some(p), Some(i))  => Some(AnsibleTask::new(p.to_string(), i.to_string(), &project_root)),
                    (_, _, _, _) => return Err(Error {
                        desc: "could not combine default and branch config to find playbook + inventory combination",
                        subject: Some(format!("branch.{}", key)),
                    })
                }
            } else { None };

            let make_task = if method == DeployMethod::Makefile {
                match lookup_as_string(table, "task") {
                    LookupResult::Missing => None,
                    LookupResult::WrongType => return Err(Error {
                        desc: "branch 'task' not a string",
                        subject: Some(format!("branch.{}.task", key)),
                    }),
                    LookupResult::Value(v) => match MakeTask::new(project_root, v) {
                        Ok(v) => Some(v),
                        Err(err) => return Err(err),
                    }
                }
            } else { None };

            if make_task.is_none() && ansible_task.is_none() {
                return Err(Error {
                    desc: "cannot construct a task for branch between local config and default",
                    subject: Some(format!("branch.{}", key)),
                })
            }

            branch.insert(key.clone(), BranchConfig {
                ansible_task: ansible_task,
                make_task: make_task,
                method: method,
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
            default_inventory: default_inventory,
            default_notify_url: default_notify_url,
            branch: branch,
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
    use super::*;
    use std::path::Path;

    #[test]
    fn test_valid_configuration() {
        // let toml = r#"
        //     [default]
        //     method = "ansible"
        //     task = "deploy"
        //     playbook = "ansible/deploy.yml"

        //     [branch.production]
        //     playbook = "ansible/production.yml"
        //     inventory = "ansible/inventory/production"

        //     [branch.staging]
        //     inventory = "ansible/inventory/staging"
        //     notify_url = "http://example.org"

        //     [branch.brian-test-branch]
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
        assert_eq!(config.default_playbook.unwrap().path(), Path::new("ansible/deploy.yml"));
        assert!(config.default_notify_url.is_none());

        // production config
        {
            let config = config.branch.get("production").unwrap();
            let ref ansible_task = config.ansible_task().unwrap();
            assert_eq!(ansible_task.playbook, "ansible/production.yml");
            assert_eq!(ansible_task.inventory, "ansible/inventory/production");
            assert_eq!(config.method, DeployMethod::Ansible);
            assert!(config.make_task.is_none());
            assert!(config.notify_url.is_none());
        }
        // staging config
        {
            let config = config.branch.get("staging").unwrap();
            let notify_url = config.notify_url.clone().unwrap();
            let ansible_task = config.ansible_task().unwrap();
            assert_eq!(ansible_task.inventory, "ansible/inventory/staging");
            assert_eq!(ansible_task.playbook, "ansible/deploy.yml");
            assert_eq!(config.method, DeployMethod::Ansible);
            assert!(config.make_task.is_none());
            assert_eq!(notify_url, "http://example.org");
        }
        // brian-test-branch config
        {
            let config = config.branch.get("brian-test-branch").unwrap();
            let method = config.method.clone();
            assert!(config.ansible_task.is_none());
            assert_eq!(method.to_string(), "makefile");
            assert!(config.notify_url.is_none());
        }

    }
}
