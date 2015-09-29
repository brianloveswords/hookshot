#[macro_use]
extern crate hyper;

extern crate deployer;
extern crate iron;
extern crate router;
extern crate rustc_serialize;
extern crate uuid;

use deployer::git::GitRepo;
use deployer::message::{SimpleMessage, GitHubMessage};
use deployer::repo_config::RepoConfig;
use deployer::signature::Signature;
use deployer::task_manager::{TaskManager, Runnable};
use iron::status;
use iron::{Iron, Request, Response};
use iron::headers::{Connection, Location};
use iron::modifiers::Header;
use router::{Router};
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

struct DeployTask {
    repo: GitRepo,
    id: Uuid,
}
impl Runnable for DeployTask {
    fn run(&mut self) {
        // clone the repo
        // read the repo config
        if let Err(git_error) = self.repo.get_latest() {
            // TODO: better error handling, this is dumb
            return println!("{}: {}", git_error.desc, String::from_utf8(git_error.output.unwrap().stderr).unwrap());
        };

        let project_root = Path::new(&self.repo.local_path);
        let config = match RepoConfig::load(&project_root) {
            // TODO: handle errors loading the repo config;
            Err(_) => return,
            Ok(config) => config,
        };
        let branch_config = match config.lookup_branch(&self.repo.branch) {
            // TODO: handle errors;
            None => return println!("No config for branch '{}'", &self.repo.branch),
            Some(config) => config,
        };

        let task = match branch_config.task() {
            // TODO: notify that there's nothing to do
            None => return println!("No task for branch '{}'", &self.repo.branch),
            Some(task) => task,
        };

        println!("[{}]: {:?}", self.id, task);

        match task.run() {
            Ok(_) => println!("[{}]: success", self.id),
            Err(e) => println!("[{}]: task failed: {}", self.id, e.desc),
        }
    }
}

const CHECKOUT_ROOT: &'static str = "/tmp/";
const HMAC_KEY: &'static str = "secret";

header! { (XHubSignature, "X-Hub-Signature") => [String] }
header! { (XSignature, "X-Signature") => [String] }


// TODO: Note that we always send Connection: close. This is a workaround for a
// bug in hyper: https://github.com/hyperium/hyper/issues/658 (link is to the
// one I filed for my specific issue which links to the ticket it's a dupe
// of). Once this is fixed we can remove the Connection::close() modifiers.
//
// In the meantime we should probably implement that Connection::close() thing
// as Iron middleware, but I don't wanna look up how to do that right now.
fn main() {
    let mut router = Router::new();
    let global_manager = Arc::new(Mutex::new(TaskManager::new()));

    router.get("/health", move |_: &mut Request| {
        Ok(Response::with((Header(Connection::close()), status::Ok, "okay")))
    });

    let shared_manager = global_manager.clone();
    router.post("/hookshot", move |req: &mut Request| {
        let task_id = Uuid::new_v4();
        println!("[{}]: request received, processing", task_id);

        println!("[{}]: loading body into string", task_id);
        let mut payload = String::new();
        if req.body.read_to_string(&mut payload).is_err() {
            println!("[{}]: could not read body into string", task_id);
            return Ok(Response::with((Header(Connection::close()), status::InternalServerError)))
        }

        // Get the signature from the header. We support both `X-Hub-Signature` and
        // `X-Signature` but they both represent the same type underneath, a
        // string. It might eventually be better to put this functionality on the
        // Signature type itself.
        println!("[{}]: looking up signature", task_id);
        let signature = {
            let possible_headers = (req.headers.get::<XSignature>(), req.headers.get::<XHubSignature>());

            let signature_string = match possible_headers {
                (Some(h), None) => h.to_string(),
                (None, Some(h)) => h.to_string(),
                (None, None) => {
                    println!("[{}]: missing signature", task_id);
                    return Ok(Response::with((Header(Connection::close()), status::Unauthorized, "missing signature")))
                },
                (Some(_), Some(_)) =>{
                    println!("[{}]: too many signatures", task_id);
                    return Ok(Response::with((Header(Connection::close()), status::Unauthorized, "too many signatures")))
                },
            };

            match Signature::from(signature_string) {
                Some(signature) => signature,
                None => {
                    println!("[{}]: could not parse signature", task_id);
                    return Ok(Response::with((Header(Connection::close()), status::Unauthorized, "could not parse signature")))
                },
            }
        };

        // Bail out if the signature doesn't match what we're expecting.
        // TODO: don't hardcode this secret, pull from `deployer` configuration
        println!("[{}]: signature found, verifying", task_id);
        if signature.verify(&payload, HMAC_KEY) == false {
            println!("[{}]: signature mismatch", task_id);
            return Ok(Response::with((Header(Connection::close()), status::Unauthorized, "signature doesn't match")))
        }

        // Try to parse the message.
        // TODO: we can be smarter about this. If we see the XHubSignature above, we
        //   should try to parse as a github message, otherwise go simple message.
        println!("[{}]: attempting to parse message from payload", task_id);
        let repo = match SimpleMessage::from(&payload) {
            Ok(message) => GitRepo::from(message, CHECKOUT_ROOT),
            Err(_) => match GitHubMessage::from(&payload) {
                Ok(message) => GitRepo::from(message, CHECKOUT_ROOT),
                Err(_) => {
                    println!("[{}]: could not parse message", task_id);
                    return Ok(Response::with((Header(Connection::close()), status::BadRequest, "could not parse message")))
                },
            },
        };

        let task = DeployTask { repo: repo, id: task_id };
        println!("[{}]: acquiring task manager lock", task_id);
        {
            let mut task_manager = shared_manager.lock().unwrap();
            let key = task_manager.ensure_queue(task.repo.branch.clone());

            println!("[{}]: attempting to schedule", task_id);
            match task_manager.add_task(&key, task) {
                Ok(_) => println!("[{}]: scheduled", task_id),
                Err(_) => {
                    println!("[{}]: could not add task to queue", task_id);
                    return Ok(Response::with((Header(Connection::close()), status::ServiceUnavailable)))
                },
            };
        }
        println!("[{}]: releasing task manager lock", task_id);
        println!("[{}]: request complete", task_id);
        let location = format!("/jobs/{}",  task_id);
        let response_body = format!("Location: {}", location);
        Ok(Response::with((
            Header(Connection::close()),
            Header(Location(location)),
            status::Accepted,
            response_body)))
    });

    println!("listening on port 4200");
    Iron::new(router).http("0.0.0.0:4200").unwrap();
    global_manager.lock().unwrap().shutdown();
}
