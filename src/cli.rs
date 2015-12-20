use deploy_task::DeployTask;
use getopts::Options;
use git::GitRepo;
use iron::headers::{Connection, Location};
use iron::modifiers::Header;
use iron::status;
use iron::{Iron, Request, Response};
use message::{SimpleMessage, GitHubMessage};
use router::Router;
use server_config::{ServerConfig, Error, Environment};
use signature::Signature;
use std::env;
use std::fmt::Display;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use task_manager::TaskManager;
use uuid::Uuid;

const ENV_CONFIG_KEY: &'static str = "HOOKSHOT_CONFIG";
const ENV_INSECURE_KEY: &'static str = "HOOKSHOT_INSECURE";

header! { (XHubSignature, "X-Hub-Signature") => [String] }
header! { (XSignature, "X-Signature") => [String] }

struct TaskStatusPrinter {
    task_id: Uuid
}
impl TaskStatusPrinter {
    fn print<T: AsRef<str> + Display>(&self, msg: T) {
        println!("[{}]: {}", self.task_id, msg);
    }
}

fn skip_signature_check() -> bool{
    match ENV_INSECURE_KEY {
        "true" | "t" | "1" => true,
        _ => false,
    }
}

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
        Ok(m) => m,
        Err(f) => {
            println!("[error]: {}", f);
            return print_usage(&program, opts);
        }
    };
    if matches.opt_present("h") {
        return print_usage(&program, opts);
    }
    if skip_signature_check() {
        println!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
        println!("hookshot is running in insecure mode, signatures will not be checked");
        println!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
    }
    let config_file = match matches.opt_str("c") {
        Some(file) => file,
        None => {
            println!("[warning]: missing --config option, looking up config by environment");
            match env::var(ENV_CONFIG_KEY) {
                Ok(file) => file,
                Err(_) => {
                    return println!("[error]: Could not load config from environment or command \
                                     line.\n\nPass --config <FILE> option or set the HOOKSHOT_CONFIG \
                                     environment variable");
                }
            }
        }
    };

    match ServerConfig::from_file(Path::new(&config_file)) {
        Ok(config) => start_server(config),
        Err(e) => match e {
            Error::FileOpenError | Error::FileReadError => {
                return println!("[error]: Error opening or reading config file {}",
                                config_file);
            }
            Error::ParseError => {
                return println!("[error]: Could not parse {}, make sure it is valid TOML",
                                config_file);
            }
            _ => {
                return println!("[error]: Could not validate file: {}", e);
            }
        },
    }
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
    let global_manager = Arc::new(Mutex::new(TaskManager::new(config.queue_limit)));

    // Create a healthcheck endpoint.
    router.get("/health", move |_: &mut Request| {
        Ok(Response::with((Header(Connection::close()), status::Ok, "okay")))
    });

    // Show the status of a specific task by UUID. If there is no log file by
    // that name or if the log file can't be read for any reason return a 404.
    let config_clone = config.clone();
    router.get("/tasks/:uuid", move |req: &mut Request| {
        let file_not_found = Ok(Response::with((Header(Connection::close()),
                                                status::NotFound,
                                                "Not Found")));

        let ref uuid = match req.extensions.get::<Router>().unwrap().find("uuid") {
            Some(query) => query,
            None => return file_not_found,
        };

        let logfile_path = Path::new(&config_clone.log_root.to_string())
            .join(format!("{}.log", uuid.to_string()));

        let mut file = match File::open(&logfile_path) {
            Ok(file) => file,
            Err(_) => return file_not_found,
        };

        let mut content = String::new();
        if let Err(_) = file.read_to_string(&mut content) {
            return file_not_found;
        };

        Ok(Response::with((Header(Connection::close()), status::Ok, content)))
    });

    // Create Webhook receiver endpoint
    let shared_manager = global_manager.clone();
    let checkout_root = config.checkout_root.to_string();
    let config_clone = config.clone();

    router.post("/tasks", move |req: &mut Request| {
        let task_id = Uuid::new_v4();
        let task_status = TaskStatusPrinter { task_id: task_id };
        let log_root = &config_clone.log_root.to_string();

        task_status.print("request received, processing");

        let mut signature = None;
        if !skip_signature_check() {
            task_status.print("looking up signature");

            // Get the signature from the header. We support both `X-Hub-Signature` and
            // `X-Signature` but they both represent the same type underneath, a
            // string. It might eventually be better to put this functionality on the
            // Signature type itself.
            signature = {
                let possible_headers = (req.headers.get::<XSignature>(),
                                        req.headers.get::<XHubSignature>());

                let signature_string = match possible_headers {
                    (Some(h), None) => h.to_string(),
                    (None, Some(h)) => h.to_string(),
                    (None, None) => {
                        task_status.print("missing signature");
                        return Ok(Response::with((Header(Connection::close()),
                                                  status::Unauthorized,
                                                  "missing signature")));
                    }
                    (Some(_), Some(_)) => {
                        task_status.print("too many signatures");
                        return Ok(Response::with((Header(Connection::close()),
                                                  status::Unauthorized,
                                                  "too many signatures")));
                    }
                };

                match Signature::from_str(&signature_string) {
                    Some(signature) => Some(signature),
                    None => {
                        task_status.print("could not parse signature");
                        return Ok(Response::with((Header(Connection::close()),
                                                  status::Unauthorized,
                                                  "could not parse signature")));
                    }
                }
            };
        }

        task_status.print("loading body into string");
        let mut payload = String::new();
        if req.body.read_to_string(&mut payload).is_err() {
            task_status.print("could not read body into string");
            return Ok(Response::with((Header(Connection::close()), status::InternalServerError)));
        }

        if !skip_signature_check() {
            // Bail out if the signature doesn't match what we're expecting.
            task_status.print("signature found, verifying");
            if signature.unwrap().verify(&payload, &config_clone.secret) == false {
                task_status.print("signature mismatch");
                return Ok(Response::with((Header(Connection::close()),
                                          status::Unauthorized,
                                          "signature doesn't match")));
            }
        }

        // Try to parse the message.
        // TODO: we can be smarter about this. If we see the XHubSignature
        // above, we should try to parse as a github message, otherwise go
        // simple message.
        task_status.print("attempting to parse message from payload");
        let repo = match SimpleMessage::from_str(&payload) {
            Ok(message) => GitRepo::from(message, &checkout_root),
            Err(_) => match GitHubMessage::from_str(&payload) {
                Ok(message) => GitRepo::from(message, &checkout_root),
                Err(_) => {
                    task_status.print("could not parse message");
                    return Ok(Response::with((Header(Connection::close()),
                                              status::BadRequest,
                                              "could not parse message")));
                }
            },
        };

        let environment = match config_clone.environment_for(&repo.owner,
                                                             &repo.name,
                                                             &repo.refstring) {
            Ok(environment) => environment,
            Err(_) => {
                task_status.print(format!("warning: error loading environment for {}, definition flawed",
                                          repo.fully_qualified_branch()));
                Environment::new()
            }
        };

        // Try to create the log file upfront to make sure we can report
        // back. If we aren't able to create it we shouldn't accept the task
        // because we will be unable to report task status.
        let logfile_path = Path::new(log_root).join(format!("{}.log", task_id.to_string()));
        let mut logfile = match File::create(&logfile_path) {
            Ok(file) => file,
            Err(e) => {
                task_status.print(format!("could not open logfile for writing: {}", e));
                return Ok(Response::with((Header(Connection::close()),
                                          status::InternalServerError)));
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

        task_status.print("acquiring task manager lock");
        {
            let mut task_manager = shared_manager.lock().unwrap();
            let key = task_manager.ensure_queue(task.repo.fully_qualified_branch());

            task_status.print("attempting to schedule");
            match task_manager.add_task(&key, task) {
                Ok(_) => task_status.print("scheduled"),
                Err(_) => {
                    task_status.print("could not add task to queue");
                    return Ok(Response::with((Header(Connection::close()),
                                              status::ServiceUnavailable)));
                }
            }
        }
        task_status.print("releasing task manager lock");
        task_status.print("request complete");

        logfile.write_all(b"task pending");

        // TODO: probably shouldn't hardcode http://, someone might want to run
        // this behind HTTPS someday.
        let location = format!("http://{}:{}/tasks/{}",
                               config_clone.hostname,
                               config_clone.port,
                               task_id);
        let response_body = format!("Location: {}", location);
        Ok(Response::with((Header(Connection::close()),
                           Header(Location(location)),
                           status::Accepted,
                           response_body)))
    });

    println!("listening on port {}", &config.port);
    let addr = format!("0.0.0.0:{}", &config.port);
    Iron::new(router).http(&*addr).unwrap();
    global_manager.lock().unwrap().shutdown();
}
