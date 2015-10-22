use hyper::header::ContentType;
use hyper::client::Client;
use rustc_serialize::json;
use ::deploy_task::DeployTask;
use ::repo_config::RepoConfig;

#[derive(RustcEncodable)]
struct Message<'a> {
    status: TaskState,
    failed: bool,
    job_url: &'a String,
    owner: &'a String,
    branch: &'a String,
    repo: &'a String,
}


#[derive(RustcEncodable)]
enum TaskState {
    Started,
    Success,
    Failed,
}

#[allow(unused_must_use)]
pub fn started(task: &DeployTask, config: &RepoConfig) {
    println!("[{}] notifier: looking up notify url", &task.id);
    let notify_url = match get_notify_url(task, config) {
        Some(url) => url,
        None => {
            println!("[{}] notifier: could not find notify url", &task.id);
            return;
        }
    };

    let repo = &task.repo;
    let job_url = format!("/jobs/{}", &task.id);
    let (branch, owner, repo_name) = (&repo.branch, &repo.owner, &repo.name);

    let message = Message {
        status: TaskState::Started,
        failed: false,
        job_url: &job_url,
        owner: owner,
        branch: branch,
        repo: repo_name,
    };

    let request_body = match json::encode(&message) {
        Ok(body) => body,
        Err(_) => return,
    };

    let client = Client::new();
    println!("[{}] notifier: sending message to {}", &task.id, &notify_url);

    match client.post(notify_url)
        .header(ContentType::json())
        .body(&request_body)
        .send() {
            Ok(_) => {},
            Err(e) => println!("[{}] notifier: could not send message {}", &task.id, e),
        }

}

pub fn success(task: &DeployTask, config: &RepoConfig) {

}

pub fn failed(task: &DeployTask, config: &RepoConfig) {

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
