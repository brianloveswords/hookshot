use ::git::GitRepo;
use ::repo_config::{RepoConfig, DeployMethod};
use ::server_config::Environment;
use ::notifier::{self};
use ::task_manager::Runnable;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use uuid::Uuid;

pub struct DeployTask {
    pub repo: GitRepo,
    pub id: Uuid,
    pub env: Environment,
    pub logdir: String,
}
impl Runnable for DeployTask {

    // TODO: this is a god damn mess and seriously needs to be refactored,
    // especially all of the logging.
    #[allow(unused_must_use)]
    fn run(&mut self) {
        let task_id = self.id.to_string();

        // Insert the checkout path for the current checkout to the environment
        self.env.insert(String::from("deployer_checkout_path"), self.repo.local_path.clone());

        // Truncate the logfile and write "task running..."
        let logfile_path = Path::new(&self.logdir).join(format!("{}.log", task_id));
        let mut logfile = match File::create(&logfile_path) {
            Ok(logfile) => logfile,
            Err(_) =>
                return println!("[{}]: could not open logfile for writing", &task_id)
        };
        logfile.write_all(b"\ntask running...\n");

        if let Err(git_error) = self.repo.get_latest() {
            let stderr = String::from_utf8(git_error.output.unwrap().stderr).unwrap();
            let err = format!("{}: {}", git_error.desc, stderr);
            logfile.write_all(format!("{}", err).as_bytes());
            return println!("[{}]: {}", task_id, err);
        };

        let project_root = Path::new(&self.repo.local_path);
        let config = match RepoConfig::load(&project_root) {
            Err(e) => {
                let err = format!("could not load config for repo {}: {} ({})",
                                  self.repo.remote_path, e.desc, e.subject.unwrap_or(String::from("")));
                logfile.write_all(format!("{}", err).as_bytes());
                return println!("[{}]: {}", &task_id, err);
            }
            Ok(config) => config,
        };

        notifier::started(&self, &config);

        let branch_config = match config.lookup_branch(&self.repo.branch) {
            None => {
                let err = format!("No config for branch '{}'", &self.repo.branch);
                logfile.write_all(format!("{}", err).as_bytes());
                return println!("[{}]: {}", &task_id, err);
            }
            Some(config) => config,
        };

        // TODO: refactor this, use a trait or something.
        let output_result = {
            match branch_config.method {
                DeployMethod::Ansible => match branch_config.ansible_task() {
                    None => {
                        let err = format!("No task for branch '{}'", &self.repo.branch);
                        logfile.write_all(format!("{}", err).as_bytes());
                        return println!("[{}]: {}", &task_id, err);
                    }
                    Some(task) => {
                        println!("[{}]: {:?}", &task_id, task);
                        println!("[{}]: with environment {:?}", &task_id, &self.env);
                        task.run(&self.env)
                    }
                },
                DeployMethod::Makefile => match branch_config.make_task() {
                    None => {
                        let err = format!("No task for branch '{}'", &self.repo.branch);
                        logfile.write_all(format!("{}", err).as_bytes());
                        return println!("[{}]: {}", &task_id, err);
                    }
                    Some(task) => {
                        println!("[{}]: {:?}", self.id, task);
                        println!("[{}]: with environment {:?}", self.id, &self.env);
                        task.run(&self.env)
                    }
                }
            }
        };

        let output = match output_result {
            Ok(output) => output,
            Err(e) => {
                let err = format!("task failed: {} ({})",
                                  e.desc, e.detail.unwrap_or(String::from("")));
                logfile.write_all(format!("{}", err).as_bytes());
                return println!("[{}]: {}", &task_id, err);
            }
        };

        let exit_code = match output.status.code() {
            None => String::from("killed"),
            Some(code) => format!("{}", code),
        };

        logfile.write_all(format!("done, exit code: {}.\n", exit_code).as_bytes());

        let exit_status = match output.status.success() {
            true => "successful",
            false => "failed",
        };
        println!("[{}]: run {}", self.id, exit_status);

        logfile.write_all(format!("{}\n", output.status).as_bytes());
        logfile.write_all(b"\n==stdout==\n");
        logfile.write_all(&output.stdout);
        logfile.write_all(b"\n==stderr==\n");
        logfile.write_all(&output.stderr);
    }
}
