use deploy_task::DeployTask;
use message::RefType;
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
    reftype: RefType,
    refstring: &'a String,
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
    let notifiers = match get_notifiers(task, config) {
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
        refstring: &repo.refstring,
        reftype: repo.reftype,
        repo: &repo.name,
    };

    let request_body = match json::encode(&message) {
        Ok(body) => body.to_owned(),
        Err(_) => return,
    };

    let client = Client::new();

    // Spawn a new thread to send the message so we don't block the task
    let task_id = task.id.clone();
    let notifiers = notifiers.clone();
    let secret = task.secret.clone();

    thread::spawn(move || {
        let sig = Signature::create(HashType::SHA256, &request_body, &secret);

        for notifiers in &notifiers {
            println!("[{}]: notifier: sending {} message to {}",
                     &task_id,
                     &status,
                     &notifiers);
            let request = client.post(notifiers)
                .header(XHookshotSignature(sig.to_string()))
                .header(ContentType::json())
                .body(&request_body)
                .send();

            if request.is_err() {
                println!("[{}]: notifier: could not send message {}",
                         &task_id,
                         &request.unwrap_err());
            }
        }
    });
}

fn get_notifiers<'a>(task: &DeployTask, config: &'a RepoConfig) -> Option<&'a Vec<String>> {
    let refstring = &task.repo.refstring;
    let reftype = task.repo.reftype;
    match config.lookup(reftype, refstring) {
        Some(refconfig) => refconfig.notifiers.as_ref(),
        None => None,
    }
}
