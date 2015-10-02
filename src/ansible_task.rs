use std::path::Path;
use std::process::{Command, Output};
use ::server_config::Environment;
use ::error::CommandError;

#[derive(Debug)]
pub struct AnsibleTask<'a> {
    playbook: String,
    inventory: String,
    project_root: &'a Path,
}

impl<'a> AnsibleTask<'a> {
    pub fn run(&self, env: &Environment) -> Result<Output, CommandError> {
        let mut command = Command::new("ansible-playbook");
        command.current_dir(&self.project_root);
        for (k, v) in env {
            command.arg("-e");
            command.arg(format!("{}={}", k, v));
        }
        command.arg("-i")
            .arg(&self.inventory)
            .arg(&self.playbook);

        match command.output() {
            Ok(r) => Ok(r),
            Err(e) => return Err(CommandError {
                desc: "failed to execute `make`, see detail",
                output: None,
                detail: Some(format!("{}", e)),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use ::server_config::Environment;
    use std::io::{self, Read};
    use std::env;
    use std::fs::File;
    use std::path::{Path, PathBuf};
    use uuid::Uuid;

    fn tmpfile() -> Result<PathBuf, io::Error> {
        Ok(try!(env::current_dir())
           .join("tmp")
           .join("deployer-test-file.txt"))
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
        };

        let mut file = match File::open(tmpfile) {
            Ok(f) => f,
            Err(_) => panic!("could not open tmpfile"),
        };
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();

        assert_eq!(format!("{} $ {}", uuid1, uuid2), contents);
    }
}
