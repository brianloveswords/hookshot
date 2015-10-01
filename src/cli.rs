use ::git::GitRepo;
use ::message::{SimpleMessage, GitHubMessage};
use ::repo_config::RepoConfig;
use ::server_config::{ServerConfig, Error, Environment};
use ::signature::Signature;
use ::task_manager::{TaskManager, Runnable};
use getopts::Options;
use iron::headers::{Connection, Location};
use iron::modifiers::Header;
use iron::status;
use iron::{Iron, Request, Response};
use router::{Router};
use std::env;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

struct DeployTask {
    repo: GitRepo,
    id: Uuid,
    env: Environment,
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
        println!("[{}]: with environment {:?}", self.id, &self.env);
        match task.run(&self.env) {
            Ok(_) => println!("[{}]: success", self.id),
            Err(e) => println!("[{}]: task failed: {}", self.id, e.desc),
        }
    }
}

const ENV_CONFIG_KEY: &'static str = "DEPLOYER_CONFIG";

header! { (XHubSignature, "X-Hub-Signature") => [String] }
header! { (XSignature, "X-Signature") => [String] }

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [options]", program);
    print!("{}", opts.usage(&brief));
}

pub fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optopt("c", "config", "configuration file to use", "FILE");
    opts.optflag("h", "help", "print this help menu");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => { m }
        Err(f) => {
            println!("[error]: {}", f.to_string());
            print_usage(&program, opts);
            return;
        }
    };
    if matches.opt_present("h") {
        println!("printing out usage");
        print_usage(&program, opts);
        return;
    }
    let config_file = match matches.opt_str("c") {
        Some(file) => file,
        None => {
            println!("[warning]: missing --config option, looking up config by environment");
            match env::var(ENV_CONFIG_KEY) {
                Ok(file) => file,
                Err(_) => {
                    println!("[error]: Could not load config from environment or command line.\n\nPass --config <FILE> option or set the DEPLOYER_CONFIG environment variable");
                    return;
                },
            }
        }
    };

    match ServerConfig::from_file(Path::new(&config_file)) {
        Ok(config) => start_server(config),
        Err(e) => match e {
            Error::FileOpenError | Error::FileReadError => {
                println!("[error]: Error opening or reading config file {}", config_file);
                return;
            },
            Error::ParseError => {
                println!("[error]: Could not parse {}, make sure it is valid TOML", config_file);
                return;
            },
            _ => {
                println!("[error]: Could not validate file: {}", e);
                return;
            }
        }
    };
}

// TODO: Note that we always send Connection: close. This is a workaround for a
// bug in hyper: https://github.com/hyperium/hyper/issues/658 (link is to the
// one I filed for my specific issue which links to the ticket it's a dupe
// of). Once this is fixed we can remove the Connection::close() modifiers.
//
// In the meantime we should probably implement that Connection::close() thing
// as Iron middleware, but I don't wanna look up how to do that right now.
fn start_server(config: ServerConfig) {
    let mut router = Router::new();
    let global_manager = Arc::new(Mutex::new(TaskManager::new()));

    // Create a healthcheck endpoint.
    router.get("/health", move |_: &mut Request| {
        Ok(Response::with((Header(Connection::close()), status::Ok, "okay")))
    });

    let port = config.port.clone();
    let checkout_root = String::from(config.checkout_root.path().to_str().unwrap());

    // Create Webhook receiver endpoint
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
        if signature.verify(&payload, &config.secret) == false {
            println!("[{}]: signature mismatch", task_id);
            return Ok(Response::with((Header(Connection::close()), status::Unauthorized, "signature doesn't match")))
        }

        // Try to parse the message.
        // TODO: we can be smarter about this. If we see the XHubSignature above, we
        //   should try to parse as a github message, otherwise go simple message.
        println!("[{}]: attempting to parse message from payload", task_id);
        let repo = match SimpleMessage::from(&payload) {
            Ok(message) => GitRepo::from(message, &checkout_root),
            Err(_) => match GitHubMessage::from(&payload) {
                Ok(message) => GitRepo::from(message,&checkout_root),
                Err(_) => {
                    println!("[{}]: could not parse message", task_id);
                    return Ok(Response::with((Header(Connection::close()), status::BadRequest, "could not parse message")))
                },
            },
        };

        let environment = match config.environment_for(&repo.owner, &repo.name, &repo.branch) {
            Ok(environment) => environment,
            Err(_) => {
                println!("[{}]: warning: error loading environment for {}, definition flawed", task_id, repo.fully_qualified_branch());
                Environment::new()
            }
        };

        let task = DeployTask { repo: repo, id: task_id, env: environment };
        println!("[{}]: acquiring task manager lock", task_id);
        {
            let mut task_manager = shared_manager.lock().unwrap();
            let key = task_manager.ensure_queue(task.repo.fully_qualified_branch());

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

    println!("listening on port {}", &port);
    let addr = format!("0.0.0.0:{}", &port);
    Iron::new(router).http(&*addr).unwrap();
    global_manager.lock().unwrap().shutdown();
}