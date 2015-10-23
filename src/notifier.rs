use deploy_task::DeployTask;
use hyper::client::Client;
use hyper::header::ContentType;
use repo_config::RepoConfig;
use rustc_serialize::json::{self, ToJson, Json};
use signature::{Signature, HashType};
use std::fmt::{self, Display, Formatter};
use std::thread;

header! { (XHookshotSignature, "X-Hookshot-Signature") => [String] }

#[derive(RustcEncodable)]
struct Message<'a> {
    status: TaskState,
    failed: bool,
    task_id: &'a String,
    task_url: &'a String,
    owner: &'a String,
    branch: &'a String,
    repo: &'a String,
    sha: &'a String,
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
    let task_url = format!("http://{}/tasks/{}", &task.host, &task.id);

    let failed = match status {
        TaskState::Failed => true,
        _ => false,
    };

    let message = Message {
        status: status.clone(),
        failed: failed,
        task_id: &format!("{}", task.id),
        task_url: &task_url,
        sha: &repo.sha,
        owner: &repo.owner,
        branch: &repo.branch,
        repo: &repo.name,
    };

    let request_body = match json::encode(&message) {
        Ok(body) => body.to_owned(),
        Err(_) => return,
    };

    let client = Client::new();
    println!("[{}]: notifier: sending {} message to {}",
             &task.id,
             &status,
             &notify_url);

    // Spawn a new thread to send the message so we don't block the task
    let task_id = task.id.clone();
    let notify_url = notify_url.clone();
    let secret = task.secret.clone();
    thread::spawn(move || {
        let sig = Signature::create(HashType::SHA256, &request_body, &secret);

        let request = client.post(&notify_url)
            .header(XHookshotSignature(sig.to_string()))
            .header(ContentType::json())
            .body(&request_body)
            .send();

        if request.is_err() {
            println!("[{}]: notifier: could not send message {}",
                     &task_id,
                     &request.unwrap_err());
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
