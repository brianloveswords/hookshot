use error::CommandError;
use rustc_serialize::json;
use server_config::Environment;
use std::path::Path;
use std::process::{Command, Output};

#[derive(Debug, Clone)]
pub struct AnsibleTask<'a> {
    pub playbook: String,
    pub inventory: String,
    pub project_root: &'a Path,
}

impl<'a> AnsibleTask<'a> {
    pub fn new(playbook: String, inventory: String, project_root: &'a Path) -> AnsibleTask {
        AnsibleTask {
            playbook: playbook,
            inventory: inventory,
            project_root: project_root,
        }
    }

    pub fn run(&self, env: &Environment) -> Result<Output, CommandError> {
        let mut command = Command::new("ansible-playbook");
        command.current_dir(&self.project_root);
        for (k, v) in env {
            command.env(k, v);
            command.arg("-e");
            // We use JSON encoding on the string as a way of making it safe for
            // use as a quoted command line variable.
            command.arg(format!("{}={}", k, json::encode(v).unwrap()));
        }
        command.arg("-i");
        command.arg(&self.inventory);
        command.arg(&self.playbook);
        match command.output() {
            Ok(r) => Ok(r),
            Err(e) => return Err(CommandError {
                desc: "failed to execute `ansible-playbook`, see detail",
                output: None,
                detail: Some(format!("{}", e)),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use server_config::Environment;
    use std::io::{self, Read};
    use std::env;
    use std::fs::File;
    use std::path::{Path, PathBuf};
    use uuid::Uuid;

    fn tmpfile() -> Result<PathBuf, io::Error> {
        Ok(try!(env::current_dir())
               .join("tmp")
               .join("hookshot-test-file.txt"))
    }

    #[test]
    fn test_run_ansible_task() {
        let test_dir = Path::new("./src/test/ansible_task");
        let ansible = AnsibleTask {
            playbook: String::from("playbook.yml"),
            inventory: String::from("inventory"),
            project_root: test_dir,
        };
        let mut env = Environment::new();
        let tmpfile = String::from(tmpfile().unwrap().to_str().unwrap());
        let (uuid1, uuid2) = (Uuid::new_v4().to_string(), Uuid::new_v4().to_string());
        env.insert(String::from("uuid1"), uuid1.clone());
        env.insert(String::from("uuid2"), uuid2.clone());
        env.insert(String::from("tmpfile"), tmpfile.clone());
        match ansible.run(&env) {
            Ok(_) => (),
            Err(_) => panic!("ansible task failed"),
        }

        let mut file = match File::open(tmpfile) {
            Ok(f) => f,
            Err(_) => panic!("could not open tmpfile"),
        };
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();

        assert_eq!(format!("{} $ {}", uuid1, uuid2), contents);
    }
}
