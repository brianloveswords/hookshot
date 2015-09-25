#![allow(dead_code)]
#![allow(unused_imports)]

use toml::{self, Value};
use std::path::Path;
use std::u16;
use ::verified_path::VerifiedPath;
use ::error::Error;

pub static DEFAULT_PORT: u16 = 5712;

#[derive(Debug)]
struct ServerConfig {
    secret: String,
    checkout_root: VerifiedPath,
    port: u16,
}
impl ServerConfig {
    pub fn from_str(string: &str) -> Result<ServerConfig, Error> {
        let root = match toml::Parser::new(string).parse() {
            Some(value) => value,
            None => return Err(Error {
                desc: "could not parse toml",
                subject: None,
            }),
        };
        let config = match root.get("config") {
            Some(value) => value,
            None => return Err(Error {
                desc: "missing 'config' section",
                subject: Some(String::from("config")),
            }),
        };
        let secret = match lookup_as_string(config, "secret") {
            LookupResult::Missing => return Err(Error {
                desc: "missing required field 'config.secret'",
                subject: Some(String::from("config.secret")),
            }),
            LookupResult::WrongType => return Err(Error {
                desc: "'config.secret' must be a string",
                subject: Some(String::from("config.secret")),
            }),
            LookupResult::Value(v) => String::from(v),
        };
        let u16_max = u16::max_value() as i64;
        let port = match config.lookup("port") {
            None => DEFAULT_PORT,
            Some(&Value::Integer(port)) if port < u16_max => port as u16,
            _ => return Err(Error {
                desc: "'config.port' must be a 16 bit integer",
                subject: Some(String::from("config.port")),
            }),
        };
        let checkout_root = match lookup_as_string(config, "checkout_root") {
            LookupResult::Missing => return Err(Error {
                desc: "missing required field 'checkout_root'",
                subject: Some(String::from("config.checkout_root")),
            }),
            LookupResult::WrongType => return Err(Error {
                desc: "'checkout_root' must be a string",
                subject: Some(String::from("config.checkout_root")),
            }),
            LookupResult::Value(v) =>
                match VerifiedPath::directory(None, Path::new(v)) {
                    Ok(v) => v,
                    Err(err) => return Err(err),
                },
        };

        Ok(ServerConfig {
            port: port,
            checkout_root: checkout_root,
            secret: secret,
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
fn lookup_as_int<'a>(obj: &'a toml::Value, key: &'static str) -> LookupResult<'a> {
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
    use super::ServerConfig;
    use tempdir::TempDir;

    #[test]
    fn test_valid_config() {
        let tmpdir = TempDir::new("deployer-server-config-test").unwrap();
        let mut toml = String::from(r#"
            [config]
            secret = "it's a secret to everyone"
            port = 5712
        "#);
        // Add a path we know exists as `checkout_root`
        let tmpdir_as_string = String::from(tmpdir.path().to_str().unwrap());
        toml.push_str(&format!("checkout_root = \"{}\"\n", tmpdir_as_string));

        let config = ServerConfig::from_str(&toml).unwrap();
        assert_eq!(config.secret, "it's a secret to everyone");
        assert_eq!(config.port, 5712u16);
        assert_eq!(config.checkout_root.path(), tmpdir_as_string);
    }

    #[test]
    fn test_valid_config_default_port() {
        let tmpdir = TempDir::new("deployer-server-config-test").unwrap();
        let mut toml = String::from(r#"
            [config]
            secret = "it's a secret to everyone"
        "#);
        // Add a path we know exists as `checkout_root`
        let tmpdir_as_string = String::from(tmpdir.path().to_str().unwrap());
        toml.push_str(&format!("checkout_root = \"{}\"\n", tmpdir_as_string));

        let config = ServerConfig::from_str(&toml).unwrap();
        assert_eq!(config.port, super::DEFAULT_PORT);
    }

    #[test]
    fn test_invalid_config_bad_checkout_path() {
        let toml = String::from(r#"
            [config]
            secret = "it's a secret to everyone"
            port = 5712
            checkout_path = "/this/does/not/exist/"
        "#);

        let config = ServerConfig::from_str(&toml);
        assert!(config.is_err());
        assert_eq!(config.err().unwrap().subject().unwrap(), "config.checkout_root");
    }

    #[test]
    fn test_invalid_config_missing_checkout_path() {
        let toml = String::from(r#"
            [config]
            secret = "it's a secret to everyone"
            port = 5712
        "#);

        let config = ServerConfig::from_str(&toml);
        assert!(config.is_err());
        assert_eq!(config.err().unwrap().subject().unwrap(), "config.checkout_root");
    }

    #[test]
    fn test_invalid_config_bad_secret() {
        let tmpdir = TempDir::new("deployer-server-config-test").unwrap();
        let mut toml = String::from(r#"
            [config]
            secret = []
            port = 5712
        "#);
        // Add a path we know exists as `checkout_root`
        let tmpdir_as_string = String::from(tmpdir.path().to_str().unwrap());
        toml.push_str(&format!("checkout_root = \"{}\"\n", tmpdir_as_string));

        let config = ServerConfig::from_str(&toml);
        assert!(config.is_err());
        assert_eq!(config.err().unwrap().subject().unwrap(), "config.secret");
    }

    #[test]
    fn test_invalid_config_missing_secret() {
        let tmpdir = TempDir::new("deployer-server-config-test").unwrap();
        let mut toml = String::from(r#"
            [config]
            port = 5712
        "#);
        // Add a path we know exists as `checkout_root`
        let tmpdir_as_string = String::from(tmpdir.path().to_str().unwrap());
        toml.push_str(&format!("checkout_root = \"{}\"\n", tmpdir_as_string));

        let config = ServerConfig::from_str(&toml);
        assert!(config.is_err());
        assert_eq!(config.err().unwrap().subject().unwrap(), "config.secret");
    }

    #[test]
    fn test_invalid_config_invalid_port() {
        let tmpdir = TempDir::new("deployer-server-config-test").unwrap();
        let mut toml = String::from(r#"
            [config]
            secret = "hi"
            port = 1000000
        "#);
        // Add a path we know exists as `checkout_root`
        let tmpdir_as_string = String::from(tmpdir.path().to_str().unwrap());
        toml.push_str(&format!("checkout_root = \"{}\"\n", tmpdir_as_string));

        let config = ServerConfig::from_str(&toml);

        assert!(config.is_err());
        assert_eq!(config.err().unwrap().subject().unwrap(), "config.port");
    }

}
