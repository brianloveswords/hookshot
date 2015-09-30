use toml::{self, Value, Table};
use std::path::Path;
use std::u16;
use std::fmt;
use std::collections::BTreeMap;
use ::verified_path::VerifiedPath;

#[derive(Debug)]
pub struct ServerConfig {
    pub secret: String,
    pub checkout_root: VerifiedPath,
    pub port: u16,
    pub environments: Table,
}

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
    InvalidEnvironmentTable,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match *self {
            Error::ParseError => "could not parse configuration",
            Error::MissingConfigSection => "missing 'config' section",
            Error::MissingSecret => "missing 'config.secret'",
            Error::InvalidSecret => "'config.secret' must be a secret",
            Error::MissingPort => "missing 'config.port'",
            Error::InvalidPort => "'config.port' must be 16 integer",
            Error::MissingCheckoutRoot => "missing 'config.checkout_root'",
            Error::InvalidCheckoutRoot => "'config.checkout_root' must be a valid existing directory",
            Error::InvalidEnvironmentTable => "'env' table is invalid, check configuration",
        })
    }
}


impl ServerConfig {
    pub fn from(string: &str) -> Result<ServerConfig, Error> {
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
            None => return Err(Error::MissingPort),
            Some(&Value::Integer(port)) if port < u16_max => port as u16,
            _ => return Err(Error::InvalidPort),
        };
        let checkout_root = match lookup_as_string(config, "checkout_root") {
            LookupResult::Missing => return Err(Error::MissingCheckoutRoot),
            LookupResult::WrongType => return Err(Error::InvalidCheckoutRoot),
            LookupResult::Value(v) =>
                match VerifiedPath::directory(None, Path::new(v)) {
                    Ok(v) => v,
                    Err(_) => return Err(Error::InvalidCheckoutRoot),
                },
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
            secret: secret,
            environments: environments,
        })
    }

    pub fn environment_for<'a>(&self, owner: &'a str, repo: &'a str, branch: &'a str) -> Result<BTreeMap<&str, String>, Error> {
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
                Some(v) => result.insert(k, String::from(v)),
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
            checkout_root = "/tmp"
        "#;

        let config = ServerConfig::from(&toml).unwrap();
        assert_eq!(config.secret, "it's a secret to everyone");
        assert_eq!(config.port, 5712u16);
        assert_eq!(config.checkout_root.path(), Path::new("/tmp"));
    }

    #[test]
    fn test_invalid_config_bad_checkout_root() {
        let toml = r#"
            [config]
            secret = "it's a secret to everyone"
            port = 5712
            checkout_root = "/this/does/not/exist/"
        "#;

        expect_error!(toml, Error::InvalidCheckoutRoot);
    }

    #[test]
    fn test_invalid_config_missing_checkout_root() {
        let toml = r#"
            [config]
            secret = "it's a secret to everyone"
            port = 5712
        "#;

        expect_error!(toml, Error::MissingCheckoutRoot);
    }

    #[test]
    fn test_invalid_config_bad_secret() {
        let toml = r#"
            [config]
            secret = {}
            port = 5712
            checkout_root = "/tmp"
        "#;

        expect_error!(toml, Error::InvalidSecret);
    }

    #[test]
    fn test_invalid_config_missing_secret() {
        let toml = r#"
            [config]
            port = 5712
            checkout_root = "/tmp"
        "#;

        expect_error!(toml, Error::MissingSecret);
    }

    #[test]
    fn test_invalid_config_invalid_port() {
        let toml = r#"
            [config]
            secret = "it's a secret to everyone"
            port = "ham sandwiches"
            checkout_root = "/tmp"
        "#;

        expect_error!(toml, Error::InvalidPort);
    }

    #[test]
    fn test_environments() {
        let toml = r#"
            [config]
            port = 1212
            secret = "it's a secret to everyone"
            checkout_root = "/tmp"

            [env.brianloveswords.deployer.master]
            username = "brianloveswords"
            repository = "deployer"
            branch = "master"

            [env.brianloveswords."d.o.t.s".overrides]
            username = "not-brianloveswords"
            repository = "not-deployer"
            branch = "overrides"
        "#;

        let config = ServerConfig::from(&toml).unwrap();

        let env1 = config.environment_for("brianloveswords", "deployer", "master").unwrap();
        assert_eq!(env1.get("username").unwrap(), "brianloveswords");
        assert_eq!(env1.get("repository").unwrap(), "deployer");
        assert_eq!(env1.get("branch").unwrap(), "master");

        // let env2 = config.environment_for("brianloveswords", "deployer", "overrides");
        // let env3 = config.environment_for("brianloveswords", "deployer", "does-not-exist");
    }

}
