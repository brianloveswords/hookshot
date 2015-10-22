use ::deploy_task::DeployTask;
use ::git::GitRepo;
use ::message::{SimpleMessage, GitHubMessage};
use ::server_config::{ServerConfig, Error, Environment};
use ::signature::Signature;
use ::task_manager::TaskManager;
use getopts::Options;
use iron::headers::{Connection, Location};
use iron::modifiers::Header;
use iron::status;
use iron::{Iron, Request, Response};
use router::{Router};
use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const ENV_CONFIG_KEY: &'static str = "HOOKSHOT_CONFIG";

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
                    println!("[error]: Could not load config from environment or command line.\n\nPass --config <FILE> option or set the HOOKSHOT_CONFIG environment variable");
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
#[allow(unused_must_use)]
fn start_server(config: ServerConfig) {
    let mut router = Router::new();
    let global_manager = Arc::new(Mutex::new(TaskManager::new()));

    // Create a healthcheck endpoint.
    router.get("/health", move |_: &mut Request| {
        Ok(Response::with((Header(Connection::close()), status::Ok, "okay")))
    });

    let config_clone = config.clone();
    router.get("/tasks/:uuid", move |req: &mut Request| {
        let file_not_found =
            Ok(Response::with((Header(Connection::close()), status::NotFound, "Not Found")));

        let ref uuid = match req.extensions.get::<Router>().unwrap().find("uuid") {
            Some(query) => query,
            None => return file_not_found,
        };

        let logfile_path = Path::new(&config_clone.log_root.to_string()).join(format!("{}.log", uuid.to_string()));

        let mut file = {
            match File::open(&logfile_path) {
                Ok(file) => file,
                Err(_) => return file_not_found,
            }
        };

        let content = {
            let mut content = String::new();
            match file.read_to_string(&mut content) {
                Ok(_) => content,
                Err(_) => return file_not_found,
            }
        };

        Ok(Response::with((Header(Connection::close()), status::Ok, content)))
    });

    // Create Webhook receiver endpoint
    let shared_manager = global_manager.clone();
    let checkout_root = config.checkout_root.to_string();
    let config_clone = config.clone();

    router.post("/hookshot", move |req: &mut Request| {
        let task_id = Uuid::new_v4();
        let log_root = &config_clone.log_root.to_string();
        println!("[{}]: request received, processing", task_id);

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

        println!("[{}]: loading body into string", task_id);
        let mut payload = String::new();
        if req.body.read_to_string(&mut payload).is_err() {
            println!("[{}]: could not read body into string", task_id);
            return Ok(Response::with((Header(Connection::close()), status::InternalServerError)))
        }

        // Bail out if the signature doesn't match what we're expecting.
        println!("[{}]: signature found, verifying", task_id);
        if signature.verify(&payload, &config_clone.secret) == false {
            println!("[{}]: signature mismatch", task_id);
            return Ok(Response::with((Header(Connection::close()), status::Unauthorized, "signature doesn't match")))
        }

        // Try to parse the message.
        // TODO: we can be smarter about this. If we see the XHubSignature
        // above, we should try to parse as a github message, otherwise go
        // simple message.
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

        let environment = match config_clone.environment_for(&repo.owner, &repo.name, &repo.branch) {
            Ok(environment) => environment,
            Err(_) => {
                println!("[{}]: warning: error loading environment for {}, definition flawed", task_id, repo.fully_qualified_branch());
                Environment::new()
            }
        };

        // Try to create the log file upfront to make sure we can report
        // back. If we aren't able to create it we shouldn't accept the task
        // because we will be unable to report task status.
        let logfile_path = Path::new(log_root).join(format!("{}.log", task_id.to_string()));
        let mut logfile = {
            match File::create(&logfile_path) {
                Ok(file) => file,
                Err(e) => {
                    println!("[{}]: could not open logfile for writing: {}", task_id, e);
                    return Ok(Response::with((Header(Connection::close()), status::InternalServerError)))
                }
            }
        };

        let task = DeployTask {
            repo: repo,
            id: task_id,
            env: environment,
            host: format!("{}:{}", &config_clone.hostname, &config_clone.port),
            logdir: config_clone.log_root.to_string(),
            secret: config_clone.secret.clone(),
        };

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

        logfile.write_all(b"task pending");

        let location = format!("/tasks/{}",  task_id);
        let response_body = format!("Location: {}", location);
        Ok(Response::with((
            Header(Connection::close()),
            Header(Location(location)),
            status::Accepted,
            response_body)))
    });

    println!("listening on port {}", &config.port);
    let addr = format!("0.0.0.0:{}", &config.port);
    Iron::new(router).http(&*addr).unwrap();
    global_manager.lock().unwrap().shutdown();
}
