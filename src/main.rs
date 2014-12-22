use std::io::{TcpListener, TcpStream};
use std::io::{Acceptor, Listener};
use std::thread::Thread;

use std::str;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:1469");

    let mut acceptor = listener.listen();

    fn handle_client(mut stream: TcpStream) {
        let peer_name = stream.peer_name().unwrap();

        match stream.write("~* welcome to the internet *~\n".as_bytes()) {
            Err(e) => println!("error writing message: {}", e),
            Ok(_) => println!("sent welcome message to {}", peer_name),
        };

        let msg = match stream.read_to_end() {
            Ok(msg) => msg,
            Err(e) => panic!("some fuckin error: {}", e),
        };
        println!("this is the message: {}",
                 str::from_utf8(msg.as_slice()).unwrap());
    }

    for stream in acceptor.incoming() {
        match stream {
            Err(e) => { /* connection failed */ },
            Ok(stream) => Thread::spawn(move|| {
                handle_client(stream)
            }).detach(),
        }
    }
    println!("Done listening, dropping acceptor");
    drop(acceptor);
}
