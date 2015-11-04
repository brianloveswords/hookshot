use ansible_task::AnsibleTask;
use make_task::MakeTask;
use std::collections::BTreeMap;
use std::error::Error as StdError;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::string::ToString;
use toml;
use verified_path::VerifiedPath;

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
    pub notify_url: Option<URL>,
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

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    FileLoad,
    FileRead,
    Parse,
    MissingDefault,
    InvalidDefaultMethod,
    InvalidDefaultMakeTask,
    InvalidDefaultPlaybook,
    InvalidDefaultInventory,
    InvalidDefaultNotifyUrl,
    MissingBranchConfig,
    InvalidBranchConfig,
    InvalidBranchEntry,
    InvalidBranchEntryMethod,
    InvalidBranchEntryPlaybook,
    InvalidBranchEntryInventory,
    InvalidBranchEntryNotifyUrl,
    InvalidBranchEntryTask,
    InvalidAnsibleConfig,
    InvalidMakeTaskConfig,
    MissingBranchTask,
}
impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::FileLoad => "could not open hookshot configuration",
            Error::FileRead => "could not read file contents",
            Error::Parse => "could not parse file as toml",
            Error::MissingDefault => "missing `default` section",
            Error::InvalidDefaultMethod => "invalid type for `default.method`, valid values are 'ansible' and 'makefile'",
            Error::InvalidDefaultMakeTask => "`default.task` must be a valid, existing make task",
            Error::InvalidDefaultPlaybook => "`default.playbook` must point to an existing file",
            Error::InvalidDefaultInventory => "`default.inventory` must point to an existing file",
            Error::InvalidDefaultNotifyUrl => "`default.notify_url` must be a URL",
            Error::MissingBranchConfig => "must configure at least one branch (missing [branch.<name>])",
            Error::InvalidBranchConfig => "`branch` must be a table",
            Error::InvalidBranchEntry => "every `branch.<name>` entry must be a table",
            Error::InvalidBranchEntryMethod => "invalid branch `method`, valid values are 'ansible' and 'makefile'",
            Error::InvalidBranchEntryPlaybook => "branch `playbook` must point to an existing file",
            Error::InvalidBranchEntryInventory => "branch `inventory` must point to an existing file",
            Error::InvalidBranchEntryNotifyUrl => "branch `notify_url` must be valid URL",
            Error::InvalidBranchEntryTask => "branch `task` must be valid, existing make task",
            Error::InvalidAnsibleConfig => "could not combine default and branch config to find playbook + inventory combination",
            Error::InvalidMakeTaskConfig => "could not combine default and branch config to find valid make task",
            Error::MissingBranchTask => "cannot construct a task for branch between local config and default",
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

#[derive(Debug)]
pub struct RepoConfig<'a> {
    default_method: DeployMethod,
    default_task: Option<MakeTask<'a>>,
    default_playbook: Option<VerifiedPath>,
    default_inventory: Option<VerifiedPath>,
    pub default_notify_url: Option<URL>,
    branch: BranchConfigMap<'a>,
    project_root: &'a Path,
}

impl<'a> RepoConfig<'a> {
    pub fn lookup_branch(&self, name: &String) -> Option<&BranchConfig<'a>> {
        self.branch.get(name)
    }

    pub fn load(project_root: &'a Path) -> Result<RepoConfig<'a>, Error> {
        let config_path = project_root.join(".hookshot.conf");
        let mut file = match File::open(&config_path) {
            Ok(file) => file,
            Err(_) => return Err(Error::FileLoad),
        };
        let mut contents = String::new();
        if file.read_to_string(&mut contents).is_err() {
            return Err(Error::FileRead);
        }
        Self::from_str(&contents, project_root)
    }

    pub fn from_str(string: &str, project_root: &'a Path) -> Result<RepoConfig<'a>, Error> {
        let root = match toml::Parser::new(string).parse() {
            Some(value) => value,
            None => return Err(Error::Parse),
        };

        let default = match root.get("default") {
            Some(value) => value,
            None => return Err(Error::MissingDefault),
        };

        let default_method = match lookup_as_string(default, "method") {
            LookupResult::Missing => DeployMethod::Makefile,
            LookupResult::WrongType => return Err(Error::InvalidDefaultMethod),
            LookupResult::Value(v) => match v {
                "ansible" => DeployMethod::Ansible,
                "makefile" | "make" => DeployMethod::Makefile,
                _ => return Err(Error::InvalidDefaultMethod),
            },
        };

        let default_task = match lookup_as_string(default, "task") {
            LookupResult::Missing => None,
            LookupResult::WrongType => return Err(Error::InvalidDefaultMakeTask),
            LookupResult::Value(v) => match MakeTask::new(project_root, v) {
                Ok(v) => Some(v),
                Err(_) => return Err(Error::InvalidDefaultMakeTask),
            },
        };

        let default_playbook = match lookup_as_string(default, "playbook") {
            LookupResult::Missing => None,
            LookupResult::WrongType => return Err(Error::InvalidDefaultPlaybook),
            LookupResult::Value(v) => match VerifiedPath::file(Some(project_root), Path::new(v)) {
                Ok(v) => Some(v),
                Err(_) => return Err(Error::InvalidDefaultPlaybook),
            },
        };

        let default_inventory = match lookup_as_string(default, "inventory") {
            LookupResult::Missing => None,
            LookupResult::WrongType => return Err(Error::InvalidDefaultInventory),
            LookupResult::Value(v) => match VerifiedPath::file(Some(project_root), Path::new(v)) {
                Ok(v) => Some(v),
                Err(_) => return Err(Error::InvalidDefaultInventory),
            },
        };

        let default_notify_url = match lookup_as_string(default, "notify_url") {
            LookupResult::Missing => None,
            LookupResult::WrongType => return Err(Error::InvalidDefaultNotifyUrl),
            LookupResult::Value(v) => Some(v.to_string()),
        };

        let raw_branch = match root.get("branch") {
            None => return Err(Error::MissingBranchConfig),
            Some(v) => match v.as_table() {
                None => return Err(Error::InvalidBranchConfig),
                Some(v) => v,
            },
        };

        let mut branch = BranchConfigMap::new();

        for (key, table) in raw_branch.iter() {
            if table.as_table().is_none() {
                return Err(Error::InvalidBranchEntry);
            }

            let method = match lookup_as_string(table, "method") {
                LookupResult::Missing => default_method,
                LookupResult::WrongType => return Err(Error::InvalidBranchEntryMethod),
                LookupResult::Value(v) => match v {
                    "ansible" => DeployMethod::Ansible,
                    "makefile" | "make" => DeployMethod::Makefile,
                    _ => return Err(Error::InvalidBranchEntryMethod),
                },
            };

            let playbook = match lookup_as_string(table, "playbook") {
                LookupResult::Missing => None,
                LookupResult::WrongType => return Err(Error::InvalidBranchEntryPlaybook),
                LookupResult::Value(v) =>
                    match VerifiedPath::file(Some(project_root), Path::new(v)) {
                        Ok(v) => Some(v),
                        Err(_) => return Err(Error::InvalidBranchEntryPlaybook),
                    },
            };
            let inventory = match lookup_as_string(table, "inventory") {
                LookupResult::Missing => None,
                LookupResult::WrongType => return Err(Error::InvalidBranchEntryInventory),
                LookupResult::Value(v) =>
                    match VerifiedPath::file(Some(project_root), Path::new(v)) {
                        Ok(v) => Some(v),
                        Err(_) => return Err(Error::InvalidBranchEntryInventory),
                    },
            };

            let notify_url = match lookup_as_string(table, "notify_url") {
                LookupResult::Missing => None,
                LookupResult::WrongType => return Err(Error::InvalidBranchEntryNotifyUrl),
                LookupResult::Value(v) => Some(v.to_string()),
            };

            let branch_make_task = match lookup_as_string(table, "task") {
                LookupResult::Missing => None,
                LookupResult::WrongType => return Err(Error::InvalidBranchEntryTask),
                LookupResult::Value(v) => match MakeTask::new(project_root, v) {
                    Ok(v) => Some(v),
                    Err(_) => return Err(Error::InvalidBranchEntryTask),
                },
            };

            let ansible_task = if method == DeployMethod::Ansible {
                // This complicated looking match tries to create a
                // playbook/inventory combination by first preferring
                // configuration for the specific branch and falling back to
                // default where necessary.
                match (playbook,
                       inventory,
                       default_playbook.clone(),
                       default_inventory.clone()) {
                    (Some(p), Some(i), _, _) |
                    (None, Some(i), Some(p), _) |
                    (Some(p), None, None, Some(i)) |
                    (None, None, Some(p), Some(i)) =>
                        Some(AnsibleTask::new(p.to_string(), i.to_string(), &project_root)),
                    (_, _, _, _) => return Err(Error::InvalidAnsibleConfig),
                }
            } else {
                None
            };

            let make_task = if method == DeployMethod::Makefile {
                match (branch_make_task, default_task.clone()) {
                    (Some(task), _) => Some(task),
                    (None, Some(task)) => Some(task),
                    (None, None) => return Err(Error::InvalidMakeTaskConfig),
                }
            } else {
                None
            };

            if make_task.is_none() && ansible_task.is_none() {
                return Err(Error::MissingBranchTask);
            }

            let branch_config = BranchConfig {
                ansible_task: ansible_task,
                make_task: make_task,
                method: method,
                notify_url: notify_url,
            };
            branch.insert(key.clone(), branch_config);
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
        assert_eq!(config.default_playbook.unwrap().path(),
                   Path::new("ansible/deploy.yml"));
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
