use ::deploy_task::DeployTask;
use ::repo_config::RepoConfig;
use hyper::client::Client;
use hyper::header::ContentType;
use rustc_serialize::json::{self, ToJson, Json};
use std::fmt::{self, Display, Formatter};
use std::thread;

#[derive(RustcEncodable)]
struct Message<'a> {
    status: TaskState,
    failed: bool,
    job_url: &'a String,
    owner: &'a String,
    branch: &'a String,
    repo: &'a String,
    task_id: &'a String,
}


#[derive(RustcEncodable, Clone)]
enum TaskState {
    Started,
    Success,
    Failed,
}

impl Display for TaskState {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", match *self {
            TaskState::Started => "started",
            TaskState::Success => "success",
            TaskState::Failed => "failed",
        })
    }
}

impl ToJson for TaskState {
    fn to_json(&self) -> Json {
        Json::String(format!("{}", self))
    }
}

#[allow(unused_must_use)]
pub fn started(task: &DeployTask, config: &RepoConfig) {
    send_message(task, config, TaskState::Started);
}

pub fn success(task: &DeployTask, config: &RepoConfig) {
    send_message(task, config, TaskState::Success);
}

pub fn failed(task: &DeployTask, config: &RepoConfig) {
    send_message(task, config, TaskState::Failed);
}

fn send_message(task: &DeployTask, config: &RepoConfig, status: TaskState) {
    println!("[{}]: notifier: looking up notify url", &task.id);
    let notify_url = match get_notify_url(task, config) {
        Some(url) => url,
        None => {
            println!("[{}]: notifier: could not find notify url", &task.id);
            return;
        }
    };

    let repo = &task.repo;
    let job_url = format!("/jobs/{}", &task.id);
    let (branch, owner, repo_name) = (&repo.branch, &repo.owner, &repo.name);

    let failed = match status {
        TaskState::Failed => true,
        _ => false,
    };

    let message = Message {
        status: status.clone(),
        failed: failed,
        job_url: &job_url,
        owner: owner,
        branch: branch,
        repo: repo_name,
        task_id: &format!("{}", task.id),
    };

    let request_body = match json::encode(&message) {
        Ok(body) => body.to_owned(),
        Err(_) => return,
    };

    let client = Client::new();
    println!("[{}]: notifier: sending {} message to {}", &task.id, &status, &notify_url);

    let task_id = task.id.clone();
    let notify_url = notify_url.clone();
    thread::spawn(move || {
        match client.post(&notify_url)
            .header(ContentType::json())
            .body(&request_body)
            .send() {
                Ok(_) => {},
                Err(e) => println!("[{}]: notifier: could not send message {}", &task_id, &e),
            }
    });
}

fn get_notify_url<'a>(task: &DeployTask, config: &'a RepoConfig) -> Option<&'a String> {
    let branch = &task.repo.branch;
    let branch_notify_url = match config.lookup_branch(branch) {
        Some(branch) => branch.notify_url.as_ref(),
        None => None,
    };

    let default_notify_url = config.default_notify_url.as_ref();

    match (branch_notify_url, default_notify_url) {
        (Some(url), _) | (None, Some(url)) => Some(url),
        (None, None) => None,
    }
}
