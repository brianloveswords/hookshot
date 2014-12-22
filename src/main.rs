use std::io::{TcpListener, TcpStream};
use std::io::{Acceptor, Listener};
use std::thread::Thread;

use std::str;
use std::os;

static DEFAULT_PORT: &'static str = "1469";

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

    let mut acceptor = listener.listen();
    println!("Listening at {}", address);

    fn handle_client(mut stream: TcpStream) {
        let peer_name = stream.peer_name().unwrap();

        stream.set_read_timeout(Some(3000));

        stream.write("~* welcome to the internet *~\n".as_bytes()).ok();

        // Read the incoming message.
        let msg = match stream.read_to_end() {
            Err(e) => panic!("Error reading incoming message: {}", e),
            Ok(msg) => msg,
        };

        // If the message is empty
        if msg.len() == 0 {
            return
        }

        println!("length of the message: {}", msg.len());

        println!("this is the message: {}",
                 str::from_utf8(msg.as_slice()).unwrap());

        stream.write("okay, message received".as_bytes()).ok();

        println!("{}: Closing connection", peer_name);
        drop(stream);
    }

    for stream in acceptor.incoming() {
        match stream {
            Err(e) => println!("MAIN: Incoming connection failed: {}", e),
            Ok(stream) => Thread::spawn(move|| {
                handle_client(stream)
            }).detach(),
        }
    }
    println!("Done listening, dropping acceptor");
    drop(acceptor);
}
