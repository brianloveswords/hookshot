use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Output};
use server_config::Environment;
use error::{Error, CommandError};

#[derive(Debug)]
pub struct MakeTask<'a> {
    task: String,
    path: &'a Path,
}

impl<'a> MakeTask<'a> {
    pub fn new(directory: &'a Path, task: &str) -> Result<MakeTask<'a>, Error> {
        let path_to_makefile = directory.join("Makefile");
        let makefile_contents = {
            let mut f = match File::open(&path_to_makefile) {
                Ok(f) => f,
                Err(_) => return Err(Error {
                    desc: "can't open Makefile",
                    subject: Some(String::from(path_to_makefile.to_str().unwrap())),
                }),
            };

            let mut contents = String::new();
            f.read_to_string(&mut contents).unwrap();
            contents
        };

        let mut task_header = task.to_string();
        task_header.push(':');

        let has_task = makefile_contents.lines().any(|line| line.starts_with(&task_header));


        match has_task {
            true => Ok(MakeTask {
                task: task.to_string(),
                path: directory,
            }),
            false => Err(Error {
                desc: "Makefile does not have specified task",
                subject: Some(task.to_string()),
            }),
        }
    }

    pub fn run(&self, env: &Environment) -> Result<Output, CommandError> {
        let mut cmd = Command::new("make");

        cmd.current_dir(&self.path);
        cmd.arg(&self.task);

        for (k, v) in env {
            cmd.env(k, v);
        }

        match cmd.output() {
            Ok(r) => Ok(r),
            Err(e) => return Err(CommandError {
                desc: "failed to execute `make`, see detail",
                output: None,
                detail: Some(format!("{}", e)),
            }),
        }
    }
}
impl<'a> ToString for MakeTask<'a> {
    fn to_string(&self) -> String {
        self.task.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::MakeTask;
    use std::path::Path;
    use server_config::Environment;

    #[test]
    fn test_run_task() {
        let test_dir = Path::new("./src/test/make_task");
        let maketask = match MakeTask::new(test_dir, "echo") {
            Ok(maketask) => maketask,
            Err(_) => panic!("should have constructed make task"),
        };
        let result = match maketask.run(&Environment::new()) {
            Ok(result) => result,
            Err(_) => panic!("should have run successfully"),
        };
        let stdout = String::from_utf8(result.stdout).unwrap();
        assert_eq!(stdout, "this passes the test\n");
    }

    #[test]
    fn test_run_task_with_env() {
        let mut env = Environment::new();
        env.insert(String::from("env"),
                   String::from("this is from the environment"));
        let test_dir = Path::new("./src/test/make_task");
        let maketask = match MakeTask::new(test_dir, "env") {
            Ok(maketask) => maketask,
            Err(_) => panic!("should have constructed make task"),
        };
        let result = match maketask.run(&env) {
            Ok(result) => result,
            Err(_) => panic!("should have run successfully"),
        };
        let stdout = String::from_utf8(result.stdout).unwrap();
        assert_eq!(stdout, "this is from the environment\n");
    }
}
