extern crate serialize;

use std::io::{TcpListener, TcpStream};
use std::io::{Acceptor, Listener};
use std::thread::Thread;
use std::str;
use std::os;

use serialize::json;

static DEFAULT_PORT: &'static str = "1469";
static DEPLOY_VIA: &'static str = "git";
static DEPLOY_KEY_SRC: &'static str = "/home/robocoup/.ssh/id_rsa";

#[deriving(Decodable, Show)]
struct RemoteCommand {
    secret: String,
    ansible: AnsibleConfig,
}

#[deriving(Decodable, Show)]
struct AnsibleConfig {
    hostname: String,
    version: String,
    optional: Option<String>,
}

#[deriving(Encodable)]
struct CommandLineVars {
    hostname: String,
    deploy_via: String,
    deploy_version: String,
    deploy_key_src: String,
}

impl CommandLineVars {
    pub fn new(hostname: String, version: String) -> CommandLineVars {
        CommandLineVars {
            hostname: hostname,
            deploy_via: String::from_str(DEPLOY_VIA),
            deploy_version: version,
            deploy_key_src: String::from_str(DEPLOY_KEY_SRC),
        }
    }
}

fn get_port() -> String {
    let default_port = String::from_str(DEFAULT_PORT);
    match os::getenv("DEPLOYER_PORT") {
        Some(val) => val,
        None => default_port,
    }
}

fn main() {
    let address = format!("0.0.0.0:{}", get_port());
    let listener = TcpListener::bind(address.as_slice());

    println!("Listening at {}", address);
    let mut acceptor = listener.listen();

    fn handle_client(mut stream: TcpStream) {
        let peer_name = stream.peer_name().unwrap();

        // Don't leave sockets lying around. If a socket doesn't send
        // data within 30 seconds, time it out.
        stream.set_read_timeout(Some(30_000));

        // Read the incoming bytes.
        let bytes = match stream.read_to_end() {
            Err(e) => panic!("Error reading incoming message: {}", e),
            Ok(bytes) => bytes,
        };

        // Turn those bytes into a string.
        let msg = str::from_utf8(bytes.as_slice()).unwrap();

        // Bail early if we don't have a message to process
        if msg.len() == 0 {
            return
        }

        let command: RemoteCommand = match json::decode(msg) {
            Ok(command) => command,
            Err(e) => panic!("Error converting message to command: {}", e)
        };

        println!("{}: {}", peer_name, command);

        stream.write("okay, message received".as_bytes()).ok();

        println!("{}: Closing connection", peer_name);
        drop(stream);
    }

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
