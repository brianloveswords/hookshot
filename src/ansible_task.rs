use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Output};
use ::server_config::Environment;
use ::error::{Error, CommandError};

#[derive(Debug)]
pub struct AnsibleTask {
    playbook: String,
    inventory: String,
    project_root: &'a Path,
}

impl AnsibleTask {
    pub fn run(env: &Environment) {
        let command = Command::new("ansible-playbook")
            .arg("-i")
            .arg(&self.inventory)
            .arg(&self.playbook)

    }
}
