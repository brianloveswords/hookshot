use ansible_task::AnsibleTask;
use message::RefType;
use make_task::MakeTask;
use std::collections::BTreeMap;
use std::error::Error as StdError;
use std::fmt;
use std::cmp::{Ordering, Ord, PartialOrd};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::string::ToString;
use regex::Regex;
use toml::{self, Table};
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

#[derive(Debug, PartialEq, Eq)]
pub struct Config<'a> {
    pub pattern: String,
    pub method: DeployMethod,
    pub notify_url: Option<URL>,
    make_task: Option<MakeTask<'a>>,
    ansible_task: Option<AnsibleTask<'a>>,
}
impl<'a> Config<'a> {
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

// We want to sort most specific branches first, so the branches with the 1) least
// amount of wildcards and 2) longest pattern.
impl<'a> PartialOrd for Config<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let self_wildcards = self.pattern.matches('*').count();
        let other_wildcards = other.pattern.matches('*').count();

        let self_len = self.pattern.len();
        let other_len = other.pattern.len();

        if self.pattern == "*" {
            return Some(Ordering::Greater)
        }
        if other.pattern == "*" {
            return Some(Ordering::Less)
        }

        match self_wildcards.cmp(&other_wildcards) {
            Ordering::Less => Some(Ordering::Less),
            Ordering::Equal if self_wildcards == 0 => self.pattern.partial_cmp(&other.pattern),
            Ordering::Equal => match self_len.cmp(&other_len) {
                Ordering::Less => Some(Ordering::Greater),
                Ordering::Equal => self.pattern.partial_cmp(&other.pattern),
                Ordering::Greater => Some(Ordering::Less),
            },
            Ordering::Greater => Some(Ordering::Greater),
        }
    }
}
impl<'a> Ord for Config<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(&other).unwrap()
    }
}

pub type ConfigMap<'a> = BTreeMap<String, Config<'a>>;

// TODO: use https://crates.io/crates/url instead
pub type URL = String;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    FileLoad,
    FileRead,
    Parse,
    InvalidDefaultMethod,
    InvalidDefaultMakeTask,
    InvalidDefaultPlaybook,
    InvalidDefaultInventory,
    InvalidDefaultNotifyUrl,
    MissingConfiguration,
    InvalidConfigGroup,
    InvalidConfigEntry(String),
    InvalidMethod(String),
    InvalidPlaybook(String),
    InvalidInventory(String),
    InvalidNotifyUrl(String),
    MissingMethod(String),
    InvalidMakeTask(String),
    MissingTask(String),
    InvalidAnsibleConfig,
    InvalidMakeTaskConfig,

}
impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::FileLoad => "could not open hookshot configuration",
            Error::FileRead => "could not read file contents",
            Error::Parse => "could not parse file as toml",
            Error::InvalidDefaultMethod => "invalid type for `default.method`, valid values are 'ansible' and 'makefile'",
            Error::InvalidDefaultMakeTask => "`default.task` must be a valid, existing make task",
            Error::InvalidDefaultPlaybook => "`default.playbook` must point to an existing file",
            Error::InvalidDefaultInventory => "`default.inventory` must point to an existing file",
            Error::InvalidDefaultNotifyUrl => "`default.notify_url` must be a URL",
            Error::MissingConfiguration => "must have at least one `branch` or `tag` entry",
            Error::InvalidConfigGroup => "`branch` or `tag` must be a table",
            Error::InvalidConfigEntry(_) => "every `branch.<pattern>` or `tag.<pattern>` entry must be a table",
            Error::InvalidMethod(_) => "invalid branch `method`, valid values are 'ansible' and 'makefile'",
            Error::InvalidPlaybook(_) => "branch `playbook` must point to an existing file",
            Error::InvalidInventory(_) => "branch `inventory` must point to an existing file",
            Error::InvalidNotifyUrl(_) => "branch `notify_url` must be valid URL",
            Error::MissingMethod(_) => "could not find `method` between default and branch config",
            Error::InvalidMakeTask(_) => "branch `task` must be valid, existing make task",
            Error::MissingTask(_) => "could not find make or ansible task between default and branch config",
            Error::InvalidAnsibleConfig => "could not find playbook + inventory between default and branch config",
            Error::InvalidMakeTaskConfig => "could not find valid make task between default and branch config",
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}
impl Error {
    pub fn related_branch(&self) -> Option<&str> {
        match *self {
            Error::MissingMethod(ref s) |
            Error::InvalidConfigEntry(ref s) |
            Error::InvalidMethod(ref s) |
            Error::InvalidPlaybook(ref s) |
            Error::InvalidInventory(ref s) |
            Error::InvalidNotifyUrl(ref s) |
            Error::InvalidMakeTask(ref s) |
            Error::MissingTask(ref s) => Some(s),
            _ => None,
        }

    }
}

#[derive(Debug)]
pub struct RepoConfig<'a> {
    branch: Option<ConfigMap<'a>>,
    tag: Option<ConfigMap<'a>>,
    project_root: &'a Path,
}

impl<'a> RepoConfig<'a> {
    pub fn lookup_branch(&self, name: &str) -> Option<&Config<'a>> {
        self.lookup(RefType::branch, name)
    }

    pub fn lookup_tag(&self, name: &str) -> Option<&Config<'a>> {
        self.lookup(RefType::tag, name)
    }

    pub fn lookup(&self, group: RefType, name: &str) -> Option<&Config<'a>> {
        let structure = {
            let possible = match group {
                RefType::branch => &self.branch,
                RefType::tag => &self.tag,
            };

            match possible {
                &None => return None,
                &Some(ref structure) => structure,
            }
        };

        if let Some(config) = structure.get(name) {
            return Some(config);
        }

        let mut catch_all = None;
        let mut wildcards = vec![];
        for (pattern, config) in structure.iter() {
            if pattern == "*" {
                catch_all = Some(config);
                continue;
            }

            if pattern.contains('*') {
                wildcards.push(config);
                continue;
            }
        }

        wildcards.sort();

        for config in &wildcards {
            let pattern = {
                let regex_string = config.pattern.replace("*", ".*?");
                match Regex::new(&format!("^{}$", regex_string)) {
                    Ok(pattern) => pattern,
                    // TODO: if there's an error we should be able to report it.
                    // This error checking should probably happen on
                    // configuration load rather than at time of lookup.
                    Err(_) => return None,
                }
            };

            if pattern.is_match(&name) {
                return Some(config);
            }
        }

        if let Some(config) = catch_all {
            return Some(config);
        }

        None
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

        let empty_table = toml::Value::Table(Table::new());
        let default = match root.get("default") {
            Some(default) => default,
            None => &empty_table,
        };

        let default_method = match lookup_as_string(default, "method") {
            LookupResult::Missing => None,
            LookupResult::WrongType => return Err(Error::InvalidDefaultMethod),
            LookupResult::Value(v) => match v {
                "ansible" => Some(DeployMethod::Ansible),
                "makefile" | "make" => Some(DeployMethod::Makefile),
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

        let mut config_groups = BTreeMap::new();

        let tag_type = "tag";
        let branch_type = "branch";
        for group_type in [tag_type, branch_type].iter() {
            let group = match root.get(group_type.to_owned()) {
                None => continue,
                Some(v) => match v.as_table() {
                    None => return Err(Error::InvalidConfigGroup),
                    Some(v) => v,
                },
            };

            config_groups.insert(group_type.clone(), ConfigMap::new());

            for (pattern, config) in group.iter() {
                if config.as_table().is_none() {
                    return Err(Error::InvalidConfigEntry(pattern.clone()));
                }

                let method = match lookup_as_string(config, "method") {
                    LookupResult::Missing => match default_method {
                        Some(method) => method,
                        None => return Err(Error::MissingMethod(pattern.clone())),
                    },
                    LookupResult::WrongType => return Err(Error::InvalidMethod(pattern.clone())),
                    LookupResult::Value(v) => match v {
                        "ansible" => DeployMethod::Ansible,
                        "makefile" | "make" => DeployMethod::Makefile,
                        _ => return Err(Error::InvalidMethod(pattern.clone())),
                    },
                };

                let playbook = match lookup_as_string(config, "playbook") {
                    LookupResult::Missing => default_playbook.clone(),
                    LookupResult::WrongType => return Err(Error::InvalidPlaybook(pattern.clone())),
                    LookupResult::Value(v) =>
                        match VerifiedPath::file(Some(project_root), Path::new(v)) {
                            Ok(v) => Some(v),
                            Err(_) => return Err(Error::InvalidPlaybook(pattern.clone())),
                        },
                };
                let inventory = match lookup_as_string(config, "inventory") {
                    LookupResult::Missing => default_inventory.clone(),
                    LookupResult::WrongType => return Err(Error::InvalidInventory(pattern.clone())),
                    LookupResult::Value(v) =>
                        match VerifiedPath::file(Some(project_root), Path::new(v)) {
                            Ok(v) => Some(v),
                            Err(_) => return Err(Error::InvalidInventory(pattern.clone())),
                        },
                };

                let notify_url = match lookup_as_string(config, "notify_url") {
                    LookupResult::Missing => default_notify_url.clone(),
                    LookupResult::WrongType => return Err(Error::InvalidNotifyUrl(pattern.clone())),
                    LookupResult::Value(v) => Some(v.to_string()),
                };

                let branch_make_task = match lookup_as_string(config, "task") {
                    LookupResult::Missing => None,
                    LookupResult::WrongType => return Err(Error::InvalidMakeTask(pattern.clone())),
                    LookupResult::Value(v) => match MakeTask::new(project_root, v) {
                        Ok(v) => Some(v),
                        Err(_) => return Err(Error::InvalidMakeTask(pattern.clone())),
                    },
                };

                let ansible_task = if method == DeployMethod::Ansible {
                    match (playbook, inventory) {
                        (Some(playbook), Some(inventory)) =>
                            Some(AnsibleTask::new(playbook.to_string(),
                                                  inventory.to_string(),
                                                  &project_root)),
                        (_, _) => return Err(Error::InvalidAnsibleConfig),
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
                    return Err(Error::MissingTask(pattern.clone()));
                }

                let config = Config {
                    pattern: pattern.clone(),
                    ansible_task: ansible_task,
                    make_task: make_task,
                    method: method,
                    notify_url: notify_url,
                };

                let mut map = config_groups.get_mut(group_type).unwrap();
                map.insert(pattern.clone(), config);
            }

        }

        Ok(RepoConfig {
            tag: config_groups.remove(&tag_type),
            branch: config_groups.remove(&branch_type),
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
    use std::error::Error as StdError;

    fn branch_config_pattern(pattern: &'static str) -> Config {
        Config {
            pattern: String::from(pattern),
            method: DeployMethod::Makefile,
            make_task: None,
            ansible_task: None,
            notify_url: None,
        }
    }

    #[test]
    fn test_valid_configuration() {
        let project_root = Path::new("./src/test/repo_config");
        let config = RepoConfig::load(project_root).unwrap();
        println!("{:?}", config);

        // production config
        {
            let config = config.lookup_branch("production").unwrap();
            let ref ansible_task = config.ansible_task().unwrap();
            assert_eq!(ansible_task.playbook, "ansible/production.yml");
            assert_eq!(ansible_task.inventory, "ansible/inventory/production");
            assert_eq!(config.method, DeployMethod::Ansible);
            assert!(config.make_task.is_none());
            assert!(config.notify_url.is_none());
        }
        // staging config
        {
            let config = config.lookup_branch("staging").unwrap();
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
            let config = config.lookup_branch("brian-test-branch").unwrap();
            let method = config.method.clone();
            assert!(config.ansible_task.is_none());
            assert_eq!(method.to_string(), "makefile");
            assert!(config.notify_url.is_none());
        }

    }

    #[test]
    fn test_branch_sorting() {
        let mut branches = vec![branch_config_pattern("branch-one"),
                                branch_config_pattern("*"),
                                branch_config_pattern("*-one-*"),
                                branch_config_pattern("branch-one-two"),
                                branch_config_pattern("branch-*"),
                                branch_config_pattern("branch*")];

        branches.sort();

        let patterns: Vec<&String> = branches.iter()
            .map(|b| &b.pattern)
            .collect();

        let expected_patterns = vec!["branch-one",
                                     "branch-one-two",
                                     "branch-*",
                                     "branch*",
                                     "*-one-*",
                                     "*"];

        assert_eq!(expected_patterns, patterns);
    }


    #[test]
    fn test_lookup_branch() {
        let toml = r#"
            [default]
            method = "make"

            [branch."*"]
            task = "build"

            [branch."prod-*"]
            task = "build"

            [branch."*-web-*"]
            task = "build"

            [branch.prod-web]
            task = "build"
        "#;
        let project_root = Path::new("./src/test/repo_config");
        let config = match RepoConfig::from_str(toml, &project_root) {
            Err(error) => {
                println!("{}", error.description());
                panic!("should be a valid config");
            },
            Ok(config) => config,
        };

        assert_eq!("prod-web", config.lookup_branch("prod-web").unwrap().pattern);
        assert_eq!("prod-*", config.lookup_branch("prod-db").unwrap().pattern);
        assert_eq!("prod-*", config.lookup_branch("prod-cats").unwrap().pattern);
        assert_eq!("*-web-*", config.lookup_branch("spider-web-lol").unwrap().pattern);
        assert_eq!("*", config.lookup_branch("catch all branch").unwrap().pattern);

    }

    #[test]
    fn test_lookup_tag() {
        let toml = r#"
            [default]
            method = "make"

            [tag."*"]
            task = "build"

            [tag."*-beta"]
            task = "build"

            [tag."v1*"]
            task = "build"

            [tag."v2*"]
            task = "build"
        "#;
        let project_root = Path::new("./src/test/repo_config");
        let config = match RepoConfig::from_str(toml, &project_root) {
            Err(error) => {
                println!("{}", error.description());
                panic!("should be a valid config");
            },
            Ok(config) => config,
        };

        assert_eq!("*-beta", config.lookup_tag("next-beta").unwrap().pattern);
        assert_eq!("*", config.lookup_tag("v3.1.2").unwrap().pattern);
        assert_eq!("v2*", config.lookup_tag("v2.1.5").unwrap().pattern);
        assert_eq!("v1*", config.lookup_tag("v1.4.5").unwrap().pattern);
        assert_eq!("*", config.lookup_tag("v901.4.5").unwrap().pattern);
    }

}
