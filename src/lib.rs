pub mod config {
    extern crate toml;

    use std::io::File;

    static DEFAULT_PORT: i64 = 1469;

    #[deriving(Show)]
    pub struct Config<'a> {
        config: toml::Table,
    }

    #[deriving(Show)]
    pub struct ConfigApp<'a>{
        app: &'a toml::Table,
        default_secret: Option<&'a str>,
    }

    #[deriving(Show)]
    pub struct ConfigError {
        desc: &'static str,
        field: Option<String>,
        detail: Option<String>,
    }

    impl<'a> Config<'a> {

        pub fn from_file(path: &'a str) -> Result<Config, ConfigError> {
            let mut file = match File::open(&Path::new(path)) {
                Ok(f) => f,
                Err(e) => return Err(ConfigError {
                    desc: "could not load file",
                    field: None,
                    detail: Some(format!("path: {}, error: {}", path, e)),
                }),
            };
            let contents: String = match file.read_to_string() {
                Ok(contents) => contents,
                Err(e) => return Err(ConfigError {
                    desc: "could not read file as utf-8",
                    field: None,
                    detail: Some(format!("path: {}, error: {}", path, e)),
                }),
            };
            Config::from_string(contents)
        }

        pub fn from_string(s: String) -> Result<Config<'a>, ConfigError> {
            let mut parser = toml::Parser::new(s.as_slice());

            match parser.parse() {
                Some(config) => Ok(Config{
                    config: config,
                }),
                None => Err(ConfigError {
                    desc: "config is not valid TOML",
                    field: None,
                    detail: None,
                }),
            }
        }

        pub fn app(&self, name: &'a str) -> Option<ConfigApp> {
            match self.config.get(name) {
                Some(app) => match app.as_table() {
                    Some(app) => Some(ConfigApp{
                        app: app,
                        default_secret: self.default_secret(),
                    }),
                    None => None,
                },
                None => None,
            }
        }

        pub fn port(&self) -> Option<i64> {
            match self.config.get("port") {
                Some(port) => port.as_integer(),
                None => Some(DEFAULT_PORT),
            }
        }

        pub fn default_secret(&'a self) -> Option<&'a str> {
            match self.config.get("default_secret") {
                Some(secret) => secret.as_str(),
                None => None,
            }
        }

        /// Validate a configuration
        pub fn validate(&self) -> Result<(), ConfigError> {
            let globals = [
                "port",
                "default_secret",
                "default_target",
                ];


            match self.config.get("port") {
                Some(port) if port.as_integer().is_none() => {
                    return Err(ConfigError {
                        desc: "`port` must be an integer",
                        field: Some("port".to_string()),
                        detail: None,
                    })
                },
                _ => { },
            };

            let default_secret = match self.config.get("default_secret") {
                Some(secret) => match secret.as_str() {
                    Some(secret) => Some(secret),
                    None => return Err(ConfigError {
                        desc: "`secret` must be a string",
                        field: Some("secret".to_string()),
                        detail: None,
                    }),
                },
                None => None,
            };

            let default_target = match self.config.get("default_target") {
                Some(target) => match target.as_str() {
                    Some(target) => Some(target),
                    None => return Err(ConfigError {
                        desc: "`target` must be a string",
                        field: Some("target".to_string()),
                        detail: None,
                    }),
                },
                None => None,
            };

            let apps = {
                let mut config = self.config.clone();
                for field in globals.iter() {
                    config.remove(&field.to_string());
                }
                config
            };

            if apps.keys().len() == 0 {
                return Err(ConfigError {
                    desc: "config must have at least 1 application",
                    field: None,
                    detail: None,
                })
            };

            let mut found_target = false;
            for (name, app) in apps.iter() {
                found_target = match default_target {
                    Some(target) if name.to_string() == target => true,
                    _ => found_target,
                };

                let definition = match app.as_table() {
                    None => return Err(ConfigError {
                        desc: "app definition must be a dictionary",
                        detail: Some(format!("'{}' must be a dictionary", name)),
                        field: Some(name.to_string()),
                    }),
                    Some(def) => def,
                };

                match definition.get("default_host") {
                    Some(host) if host.as_str().is_none() => {
                        return Err(ConfigError {
                            desc: "`default_host` must be a string",
                            detail: Some(format!("'{}.default_string' is invalid", name)),
                            field: Some(format!("{}.default_string", name)),
                        })
                    },
                    _ => {}
                };

                match definition.get("secret") {
                    None => {
                        if default_secret.is_none() {
                            return Err(ConfigError {
                                desc: "`secret` must be set for every app if there is no `default_secret`",
                                detail: Some(format!("'{}.secret' is missing", name)),
                                field: Some(format!("{}.secret", name)),
                            })
                        }
                    },
                    Some(secret) => match secret.as_str() {
                        None => return Err(ConfigError {
                            desc: "`secret` must be a string",
                            detail: Some(format!("'{}.secret' is invalid", name)),
                            field: Some(format!("{}.secret", name)),
                        }),
                        Some(_) => {},
                    },
                };

                let default_playbook = match definition.get("default_playbook") {
                    Some(playbook) => match playbook.as_str() {
                        Some(playbook) => Some(playbook),
                        None => return Err(ConfigError {
                            desc: "`default_secret` must be a string if set",
                            detail: Some(format!("'{}.default_secret' is invalid", name)),
                            field: Some(format!("{}.default_secret", name)),
                        }),
                    },
                    None => None,
                };

                let playbooks = match definition.get("playbooks") {
                    None => return Err(ConfigError {
                        desc: "there must be a `playbooks` section for every app",
                        detail: Some(format!("'{}.playbooks' is missing", name)),
                        field: Some(format!("{}.playbooks", name)),
                    }),
                    Some(playbooks) => match playbooks.as_table() {
                        None => return Err(ConfigError {
                            desc: "`playbooks` must be a dictionary",
                            detail: Some(format!("'{}.playbooks' is mising", name)),
                            field: Some(format!("{}.playbooks", name)),
                        }),
                        Some(playbooks) => playbooks,
                    },
                };

                if playbooks.keys().len() == 0 {
                    return Err(ConfigError {
                        desc: "`playbooks` must have one entry",
                        detail: Some(format!("'{}.playbooks' must have at least one entry", name)),
                        field: Some(format!("{}.playbooks", name)),
                    })
                }

                let mut found_playbook = false;
                for (key, value) in playbooks.iter() {
                    let path = match value.as_str() {
                        None => return Err(ConfigError {
                            desc: "entries in `playbooks` must be a strings",
                            detail: Some(format!("'{}.playbooks.{}' is not a string", name, key)),
                            field: Some(format!("{}.playbooks.{}", name, key)),
                        }),
                        Some(path) => path,
                    };

                    if !Path::new(path).is_absolute() {
                        return Err(ConfigError {
                            desc: "entries in `playbooks` must be absolute paths",
                            detail: Some(format!("'{}.playbooks.{}' is not an absolute path", name, key)),
                            field: Some(format!("{}.playbooks.{}", name, key)),
                        })
                    }

                    found_playbook = match default_playbook {
                        Some(playbook) if key.to_string() == playbook => true,
                        _ => found_playbook,
                    };
                }

                if default_playbook.is_some() && !found_playbook {
                    return Err(ConfigError {
                        desc: "`default_playbook` must be defined in `playbooks`",
                        detail: Some(format!("'{}.default_playbook' = '{}', which does not match a listed playbook ({})",
                                             name, default_playbook.unwrap(), playbooks)),
                        field: Some(format!("{}.default_playbook", name)),
                    })
                }
            }

            if default_target.is_some() && !found_target {
                return Err(ConfigError {
                    desc: "`default_target` is set, but doesn't match any defined applications",
                    detail: Some(format!("'{}' is not a defined application", default_target.unwrap())),
                    field: Some("default_target".to_string()),
                })
            }

            Ok(())
        }
    }

    impl<'a> ConfigApp<'a> {
        /// Test a provided secret against the configuration. Looks for the
        /// application specific secret first before falling back to the
        /// `default_secret` if that's defined. If there is a mismatch, or
        /// there are no secrets defined, returns false.
        pub fn confirm_secret(&self, provided: &'a str) -> bool {
            match self.app.get("secret") {
                Some(secret) => match secret.as_str() {
                    None => false,
                    Some(secret) => secret == provided,
                },
                None => match self.default_secret {
                    None => false,
                    Some(secret) => secret == provided,
                },
            }
        }

        /// Get the full path of a playbook from the configuration.
        pub fn playbook(&'a self, name: &'a str) -> Option<&'a str> {
            match self.app.get("playbooks") {
                None => None,
                Some(playbooks) => match playbooks.lookup(name) {
                    None => None,
                    Some(playbook) => playbook.as_str(),
                }
            }
        }

        /// Get the default playbook for the application if it exists.
        pub fn default_playbook(&'a self) -> Option<&'a str> {
            match self.app.get("default_playbook") {
                None => None,
                Some(name) => match name.as_str() {
                    None => None,
                    Some(name) => self.playbook(name)
                }
            }
        }
    }

    impl ConfigError {
        pub fn description(&self) -> &'static str {
            self.desc
        }
        pub fn field(&self) -> Option<String> {
            self.field.clone()
        }
        pub fn detail(&self) -> Option<String> {
            self.detail.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::config::Config;

    fn load_basic_config<'a>() -> Config<'a> {
        let config_string = r#"
            port = 5000

            default_secret = "default secret"

            [test-app]
            secret = "test app secret"
            default_playbook = "deploy"

            [test-app.playbooks]
            deploy = "/test-app/deploy.yml"
            provision = "/test-app/provision.yml"

            [no-secret.playbooks]
            sports = "/no-secret/sports.yml"
        "#;

        Config::from_string(config_string.to_string()).unwrap()
    }

    #[test]
    fn test_basic_config() {
        let c = load_basic_config();
        assert_eq!(5000, c.port().unwrap());
        assert_eq!("default secret", c.default_secret().unwrap());
    }

    #[test]
    fn test_app_secrets() {
        let c = load_basic_config();
        let app = c.app("test-app").unwrap();

        assert_eq!(false, app.confirm_secret("not correct"));
        assert_eq!(false, app.confirm_secret("default secret"));
        assert_eq!(true, app.confirm_secret("test app secret"));

        let no_secret = c.app("no-secret").unwrap();
        assert_eq!(true, no_secret.confirm_secret("default secret"));
    }

    #[test]
    fn test_app_playbooks() {
        let c = load_basic_config();
        let app = c.app("test-app").unwrap();

        assert_eq!("/test-app/deploy.yml", app.default_playbook().unwrap());
        assert_eq!("/test-app/deploy.yml", app.playbook("deploy").unwrap());
        assert_eq!("/test-app/provision.yml", app.playbook("provision").unwrap());
    }

    #[test]
    fn test_validation_good() {
        let c = Config::from_string(r#"
            [app]
              secret = "shhh"
            [app.playbooks]
              a = "/path/to/playbook.yml"
        "#.to_string()).unwrap();

        assert!(c.validate().is_ok());
    }

    #[test]
    fn test_validation_missing_secret() {
        let c = Config::from_string(r#"
            [app]
            [app.playbooks]
              a = "/path/to/playbook.yml"
        "#.to_string()).unwrap();

        let err = c.validate().err().unwrap();
        assert_eq!("app.secret", err.field().unwrap());
    }

    #[test]
    fn test_validation_good_with_default_secret() {
        let c = Config::from_string(r#"
            default_secret = "hi five"
            [app]
            [app.playbooks]
              a = "/path/to/playbook.yml"
        "#.to_string()).unwrap();
        assert!(c.validate().is_ok());
    }

    #[test]
    fn test_validation_bad_default_target() {
        let c = Config::from_string(r#"
            default_secret = "hi five"
            default_target = "not-a-real-app"
            [app]
            [app.playbooks]
              a = "/path/to/playbook.yml"
        "#.to_string()).unwrap();

        let err = c.validate().err().unwrap();
        assert_eq!("default_target", err.field().unwrap());
    }

    #[test]
    fn test_validation_good_default_target() {
        let c = Config::from_string(r#"
            default_secret = "hi five"
            default_target = "app"
            [app]
            [app.playbooks]
              a = "/path/to/playbook.yml"
        "#.to_string()).unwrap();
        assert!(c.validate().is_ok());
    }

    #[test]
    fn test_validation_bad_missing_apps() {
        let c = Config::from_string(r#"
        "#.to_string()).unwrap();

        let err = c.validate().err().unwrap();
        // I don't really like comparing to the description, but there
        // isn't a better option at the moment.
        assert_eq!("config must have at least 1 application", err.description());
    }

    #[test]
    fn test_validation_missing_playbooks() {
        let c = Config::from_string(r#"
            [app]
              secret = "hi five"
        "#.to_string()).unwrap();

        let err = c.validate().err().unwrap();
        assert_eq!("app.playbooks", err.field().unwrap());
    }

    #[test]
    fn test_validation_bad_default_playbook() {
        let c = Config::from_string(r#"
            [app]
              secret = "hi five"
              default_playbook = "does not exist"
            [app.playbooks]
              a = "/path/to/playbook.yml"
        "#.to_string()).unwrap();

        let err = c.validate().err().unwrap();
        assert_eq!("app.default_playbook", err.field().unwrap());
    }

    #[test]
    fn test_validation_good_default_playbook() {
        let c = Config::from_string(r#"
            [app]
              secret = "hi five"
              default_playbook = "a"
            [app.playbooks]
              a = "/path/to/playbook.yml"
        "#.to_string()).unwrap();

        assert!(c.validate().is_ok());
    }

    #[test]
    fn test_validation_bad_playbook_path() {
        let c = Config::from_string(r#"
            [app]
              secret = "hi five"
            [app.playbooks]
              bad_path = "not-absolute"
        "#.to_string()).unwrap();

        let err = c.validate().err().unwrap();
        assert_eq!("app.playbooks.bad_path", err.field().unwrap());
    }

}
