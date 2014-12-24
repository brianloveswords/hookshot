extern crate "rustc-serialize" as rustc_serialize;

use std::io::{TcpListener, TcpStream};
use std::io::{Acceptor, Listener};
use std::io::process::Command;

use std::thread::Thread;
use std::collections::BTreeMap;

use std::str;
use std::os;

use rustc_serialize::json;

static DEFAULT_PORT: &'static str = "1469";
static ANSIBLE_CMD: &'static str = "ansible-playbook";

static SECRET_ENV_KEY: &'static str = "DEPLOYER_SECRET";
static PLAYBOOK_ENV_KEY: &'static str = "DEPLOYER_PLAYBOOK";

type Object = BTreeMap<String, String>;

#[deriving(RustcDecodable, Show)]
struct RemoteCommandMsg {
    secret: String,
    config: Object,
}

fn get_from_env_or_panic(key: &str) -> String {
    match os::getenv(key) {
        Some(val) => val,
        None => panic!("Must have {} set in environment", key),
    }
}

fn get_from_env_or_default(key: &str, default: &str) -> String {
    match os::getenv(key) {
        Some(val) => val,
        None => String::from_str(default),
    }
}

fn get_port() -> String {
    get_from_env_or_default("DEPLOYER_PORT", DEFAULT_PORT)
}

fn handle_client(mut stream: TcpStream) {
    let peer_name = stream.peer_name().unwrap();
    let secret = get_from_env_or_panic(SECRET_ENV_KEY);
    let playbook = get_from_env_or_panic(PLAYBOOK_ENV_KEY);

    // Don't leave sockets lying around. If a socket doesn't send
    // data within 30 seconds, time it out.
    stream.set_read_timeout(Some(30_000));

    // Don't buffer data, send everything immediately
    stream.set_nodelay(true).ok();

    // Read the incoming bytes.
    let bytes = match stream.read_to_end() {
        Err(e) => panic!("Error reading incoming message: {}", e),
        Ok(bytes) => bytes,
    };

    // Bail early if we don't have a message to process
    if bytes.len() == 0 {
        return
    }

    // json::decode requires &str
    let msg = str::from_utf8(bytes.as_slice()).unwrap();

    // Decode the incoming message or panic
    let command: RemoteCommandMsg = match json::decode(msg) {
        Ok(command) => command,
        Err(e) => {
            stream.write("error, could not parse message".as_bytes()).ok();
            panic!("Error converting message to command: {}", e)
        }
    };

    if command.secret != secret {
        stream.write("error, wrong secret".as_bytes()).ok();
        panic!("Wrong secret");
    }

    stream.write("okay, message received\n".as_bytes()).ok();
    println!("{}: {}", peer_name, command);

    // Start a detached ansible process and set up the cli args
    let mut ansible = Command::new(ANSIBLE_CMD);
    ansible.detached();
    ansible.arg("--connection=local");
    ansible.arg("-i").arg("127.0.0.1,");
    ansible.arg("-e").arg(json::encode(&command.config));
    ansible.arg(playbook);

    println!("{}: spawning ansible", peer_name);

    let mut child = match ansible.spawn() {
        Err(why) => {
            stream.write("error, could not spawn ansible-playbook".as_bytes()).ok();
            panic!("Could not spawn `ansible-playbook`: {}", why)
        },
        Ok(child) => child
    };

    // Create a new short-lived scope to borrow a mutable reference to
    // `child` or else when we try to do `child.wait()` later the
    // compiler will get mad at us.
    {
        let mut stdout = child.stdout.as_mut().unwrap();
        loop {
            match stdout.read_byte() {
                Ok(byte) => {
                    stream.write(&[byte]).ok();
                    stream.flush().ok();
                } ,
                Err(_) => { break }
            }
        }
    }

    let stderr = child.stderr.as_mut().unwrap().read_to_end();
    stream.write(stderr.unwrap().as_slice()).ok();

    let exit_status = child.wait().unwrap();
    stream.write(format!("{}\n", exit_status).as_bytes()).ok();

    println!("{}: Closing connection", peer_name);

    stream.write("okay, see ya later!\n".as_bytes()).ok();
    drop(stream);
}

fn main() {
    // We use these next two lines to panic early if the environment
    // isn't properly set up.
    get_from_env_or_panic(SECRET_ENV_KEY);
    get_from_env_or_panic(PLAYBOOK_ENV_KEY);

    let address = format!("0.0.0.0:{}", get_port());
    let listener = TcpListener::bind(address.as_slice());

    println!("Listening at {}", address);
    let mut acceptor = listener.listen();

    for stream in acceptor.incoming() {
        match stream {
            Err(e) => panic!("Listening failed: {}", e),
            Ok(stream) => Thread::spawn(move|| {
                handle_client(stream)
            }).detach(),
        }
    }
    println!("Done listening, dropping acceptor");
    drop(acceptor);
}
