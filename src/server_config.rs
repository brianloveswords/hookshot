use std::collections::BTreeMap;
use std::error::Error as StdError;
use std::env;
use std::fmt;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::u16;
use toml::{self, Value, Table};
use verified_path::VerifiedPath;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub secret: String,
    pub hostname: String,
    pub checkout_root: VerifiedPath,
    pub log_root: VerifiedPath,
    pub port: u16,
    pub environments: Table,
}

pub type Environment = BTreeMap<String, String>;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    ParseError,
    MissingConfigSection,
    MissingSecret,
    InvalidSecret,
    MissingPort,
    InvalidPort,
    MissingCheckoutRoot,
    InvalidCheckoutRoot,
    MissingLogRoot,
    InvalidLogRoot,
    MissingHostname,
    InvalidHostname,
    InvalidEnvironmentTable,
    FileOpenError,
    FileReadError,
    DirectoryCreateError,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::ParseError => "could not parse configuration",
            Error::MissingConfigSection => "missing 'config' section",
            Error::MissingSecret => "missing 'config.secret'",
            Error::InvalidSecret => "'config.secret' must be a string",
            Error::MissingHostname => "missing 'config.hostname'",
            Error::InvalidHostname => "'config.hostname' must be a string",
            Error::MissingPort => "missing 'config.port'",
            Error::InvalidPort => "'config.port' must be 16 integer",
            Error::MissingCheckoutRoot => "missing 'config.checkout_root'",
            Error::InvalidCheckoutRoot => "'config.checkout_root' must be a directory",
            Error::MissingLogRoot => "missing 'config.log_root'",
            Error::InvalidLogRoot => "'config.log_root' must be a directory",
            Error::InvalidEnvironmentTable => "'env' table is invalid, check configuration",
            Error::FileOpenError => "could not open config file",
            Error::FileReadError => "could not read config file into string",
            Error::DirectoryCreateError => "could not create default directory",
        }
    }
}

// See http://standards.freedesktop.org/basedir-spec/basedir-spec-latest.html
fn get_xdg_data_home() -> Option<String> {
    let empty_string = String::from("");

    let home_dir = match env::var("HOME") {
        Ok(dir) => dir,
        Err(_) => return None,
    };

    Some(match env::var("XDG_DATA_HOME") {
        Ok(ref dir) if *dir == empty_string => home_dir,
        Err(_) => format!("{}/.local/share", home_dir),
        Ok(dir) => dir,
    })
}

fn get_default_log_dir() -> Option<String> {
    let xdg_data_home = match get_xdg_data_home() {
        None => return None,
        Some(dir) => dir,
    };
    Some(format!("{}/hookshot/logs", xdg_data_home))
}

fn get_default_checkout_dir() -> Option<String> {
    let xdg_data_home = match get_xdg_data_home() {
        None => return None,
        Some(dir) => dir,
    };
    Some(format!("{}/hookshot/checkouts", xdg_data_home))
}

impl ServerConfig {
    pub fn from_file(config_path: &Path) -> Result<ServerConfig, Error> {
        let mut file = match File::open(&config_path) {
            Ok(file) => file,
            Err(_) => return Err(Error::FileOpenError),
        };
        let mut contents = String::new();
        if file.read_to_string(&mut contents).is_err() {
            return Err(Error::FileReadError);
        }
        Self::from(&contents)
    }

    pub fn from(string: &str) -> Result<ServerConfig, Error> {
        let default_port = 1469;
        let default_checkout_dir = get_default_checkout_dir();
        let default_log_dir = get_default_log_dir();

        let root = match toml::Parser::new(string).parse() {
            Some(value) => value,
            None => return Err(Error::ParseError),
        };
        let config = match root.get("config") {
            Some(value) => value,
            None => return Err(Error::MissingConfigSection),
        };
        let secret = match lookup_as_string(config, "secret") {
            LookupResult::Missing => return Err(Error::MissingSecret),
            LookupResult::WrongType => return Err(Error::InvalidSecret),
            LookupResult::Value(v) => String::from(v),
        };
        let u16_max = u16::max_value() as i64;
        let port = match config.lookup("port") {
            None => default_port,
            Some(&Value::Integer(port)) if port < u16_max => port as u16,
            _ => return Err(Error::InvalidPort),
        };

        let checkout_root = match lookup_as_string(config, "checkout_root") {
            LookupResult::Missing => {
                let checkout_root_string = match default_checkout_dir {
                    None => return Err(Error::MissingCheckoutRoot),
                    Some(dir) => dir,
                };
                let checkout_root = Path::new(&checkout_root_string);
                if let Err(_) = fs::create_dir_all(&checkout_root) {
                    return Err(Error::DirectoryCreateError);
                }
                match VerifiedPath::directory(None, checkout_root) {
                    Ok(v) => v,
                    Err(_) => return Err(Error::InvalidCheckoutRoot),
                }
            }
            LookupResult::WrongType => return Err(Error::InvalidCheckoutRoot),
            LookupResult::Value(v) => match VerifiedPath::directory(None, Path::new(v)) {
                Ok(v) => v,
                Err(_) => return Err(Error::InvalidCheckoutRoot),
            },
        };

        let log_root = match lookup_as_string(config, "log_root") {
            LookupResult::Missing => {
                let log_root_string = match default_log_dir {
                    None => return Err(Error::MissingLogRoot),
                    Some(dir) => dir,
                };
                let log_root = Path::new(&log_root_string);
                if let Err(_) = fs::create_dir_all(&log_root) {
                    return Err(Error::DirectoryCreateError);
                }
                match VerifiedPath::directory(None, log_root) {
                    Ok(v) => v,
                    Err(_) => return Err(Error::InvalidLogRoot),
                }
            }
            LookupResult::WrongType => return Err(Error::InvalidLogRoot),
            LookupResult::Value(v) => match VerifiedPath::directory(None, Path::new(v)) {
                Ok(v) => v,
                Err(_) => return Err(Error::InvalidLogRoot),
            },
        };
        let hostname = match lookup_as_string(config, "hostname") {
            LookupResult::Missing => return Err(Error::MissingHostname),
            LookupResult::WrongType => return Err(Error::InvalidHostname),
            LookupResult::Value(v) => String::from(v),
        };
        let environments = match root.get("env") {
            None => Table::new(),
            Some(value) => match value.as_table() {
                None => return Err(Error::InvalidEnvironmentTable),
                Some(table) => table.clone(),
            },
        };

        Ok(ServerConfig {
            port: port,
            checkout_root: checkout_root,
            log_root: log_root,
            secret: secret,
            environments: environments,
            hostname: hostname,
        })
    }

    pub fn environment_for<'a>(&self,
                               owner: &'a str,
                               repo: &'a str,
                               branch: &'a str)
                               -> Result<Environment, Error> {
        let mut result = BTreeMap::new();

        let owner_table = match self.environments.get(owner) {
            None => return Ok(result),
            Some(value) => match value.as_table() {
                None => return Err(Error::InvalidEnvironmentTable),
                Some(table) => table,
            },
        };

        let repo_table = match owner_table.get(repo) {
            None => return Ok(result),
            Some(value) => match value.as_table() {
                None => return Err(Error::InvalidEnvironmentTable),
                Some(table) => table,
            },
        };

        let branch_table = match repo_table.get(branch) {
            None => return Ok(result),
            Some(value) => match value.as_table() {
                None => return Err(Error::InvalidEnvironmentTable),
                Some(table) => table,
            },
        };

        for (k, v) in branch_table {
            match v.as_str() {
                Some(v) => result.insert(k.clone(), String::from(v)),
                None => return Err(Error::InvalidEnvironmentTable),
            };
        }

        Ok(result)
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
    use std::env;

    macro_rules! expect_error {
        ( $i:ident, $error:path ) => {{
            let config = ServerConfig::from($i);
            assert!(config.is_err());
            assert_eq!(config.err().unwrap(), $error);
        }}
    }

    #[test]
    fn test_valid_config() {
        let toml = r#"
            [config]
            secret = "it's a secret to everyone"
            port = 5712
            hostname = "127.0.0.1"
            checkout_root = "/tmp"
            log_root = "/tmp"
        "#;
        let config = ServerConfig::from(&toml).unwrap();
        assert_eq!(config.secret, "it's a secret to everyone");
        assert_eq!(config.port, 5712u16);
        assert_eq!(config.checkout_root.path(), Path::new("/tmp"));
    }

    #[test]
    fn test_invalid_config_missing_hostname() {
        let toml = r#"
            [config]
            secret = "it's a secret to everyone"
            port = 5712
            checkout_root = "/tmp"
            log_root = "/tmp"
        "#;
        expect_error!(toml, Error::MissingHostname);
    }

    #[test]
    fn test_invalid_config_invalid_hostname() {
        let toml = r#"
            [config]
            secret = "it's a secret to everyone"
            port = 5712
            hostname = []
            checkout_root = "/tmp"
            log_root = "/tmp"
        "#;
        expect_error!(toml, Error::InvalidHostname);
    }

    #[test]
    fn test_invalid_config_bad_checkout_root() {
        let toml = r#"
            [config]
            secret = "it's a secret to everyone"
            port = 5712
            hostname = "127.0.0.1"
            log_root = "/tmp"
            checkout_root = "/this/does/not/exist/"
        "#;
        expect_error!(toml, Error::InvalidCheckoutRoot);
    }

    #[test]
    fn test_invalid_config_missing_checkout_root() {
        env::set_var("XDG_DATA_HOME", "/tmp");
        let toml = r#"
            [config]
            secret = "it's a secret to everyone"
            port = 5712
            hostname = "127.0.0.1"
            log_root = "/tmp"
        "#;
        let config = ServerConfig::from(&toml).unwrap();
        assert_eq!(config.checkout_root.path(), Path::new("/tmp/hookshot/checkouts"));
    }

    #[test]
    fn test_invalid_config_bad_secret() {
        let toml = r#"
            [config]
            secret = {}
            port = 5712
            hostname = "127.0.0.1"
            checkout_root = "/tmp"
            log_root = "/tmp"
        "#;
        expect_error!(toml, Error::InvalidSecret);
    }

    #[test]
    fn test_invalid_config_missing_secret() {
        let toml = r#"
            [config]
            port = 5712
            hostname = "127.0.0.1"
            checkout_root = "/tmp"
            log_root = "/tmp"
        "#;
        expect_error!(toml, Error::MissingSecret);
    }

    #[test]
    fn test_config_default_log_root() {
        env::set_var("XDG_DATA_HOME", "/tmp");
        let toml = r#"
            [config]
            port = 5712
            hostname = "127.0.0.1"
            secret = "shh"
            checkout_root = "/tmp"
        "#;
        let config = ServerConfig::from(&toml).unwrap();
        assert_eq!(config.log_root.path(), Path::new("/tmp/hookshot/logs"));
    }


    #[test]
    fn test_invalid_config_invalid_log_root() {
        let toml = r#"
            [config]
            port = 5712
            hostname = "127.0.0.1"
            secret = "shh"
            checkout_root = "/tmp"
            log_root = "/path/does/not/exist"
        "#;
        expect_error!(toml, Error::InvalidLogRoot);
    }

    #[test]
    fn test_invalid_config_invalid_port() {
        let toml = r#"
            [config]
            secret = "it's a secret to everyone"
            port = "ham sandwiches"
            hostname = "127.0.0.1"
            checkout_root = "/tmp"
            log_root = "/tmp"
        "#;
        expect_error!(toml, Error::InvalidPort);
    }

    #[test]
    fn test_config_default_port() {
        let toml = r#"
            [config]
            secret = "it's a secret to everyone"
            hostname = "127.0.0.1"
            checkout_root = "/tmp"
            log_root = "/tmp"
        "#;
        let config = ServerConfig::from(&toml).unwrap();
        assert_eq!(config.port, 1469);
    }

    #[test]
    fn test_environments() {
        let toml = r#"
            [config]
            port = 1212
            secret = "it's a secret to everyone"
            checkout_root = "/tmp"
            log_root = "/tmp"
            hostname = "127.0.0.1"

            [env.brianloveswords.hookshot.master]
            username = "brianloveswords"
            repository = "hookshot"
            branch = "master"

            [env.brianloveswords."d.o.t.s".overrides]
            username = "not-brianloveswords"
            repository = "not-hookshot"
            branch = "overrides"
        "#;
        let config = ServerConfig::from(&toml).unwrap();

        let env1 = config.environment_for("brianloveswords", "hookshot", "master").unwrap();
        assert_eq!(env1.get("username").unwrap(), "brianloveswords");
        assert_eq!(env1.get("repository").unwrap(), "hookshot");
        assert_eq!(env1.get("branch").unwrap(), "master");

        let env2 = config.environment_for("brianloveswords", "d.o.t.s", "overrides").unwrap();
        assert_eq!(env2.get("username").unwrap(), "not-brianloveswords");
        assert_eq!(env2.get("repository").unwrap(), "not-hookshot");
        assert_eq!(env2.get("branch").unwrap(), "overrides");
    }

}
