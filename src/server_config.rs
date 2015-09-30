use toml::{self, Value};
use std::path::Path;
use std::u16;
use std::fmt;
use ::verified_path::VerifiedPath;

#[derive(Debug)]
pub struct ServerConfig {
    pub secret: String,
    pub checkout_root: VerifiedPath,
    pub port: u16,
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
        })
    }
}


impl ServerConfig {
    pub fn from_str(string: &str) -> Result<ServerConfig, Error> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    macro_rules! expect_error {
        ( $i:ident, $error:path ) => {{
            let config = ServerConfig::from_str($i);
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

        let config = ServerConfig::from_str(&toml).unwrap();
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

}
