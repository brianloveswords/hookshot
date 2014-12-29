// use rustc_serialize::{Decodable, Decoder};
use rustc_serialize::json;
use rustc_serialize::json::DecodeResult;
use std::collections::BTreeMap;
use std::string;

type ObjectConfig = BTreeMap<String, String>;
type StringConfig = string::String;

#[deriving(RustcDecodable, Show)]
struct MessageWithStringConfig {
    secret: String,
    host: Option<String>,
    config: StringConfig,
}

#[deriving(RustcDecodable, Show)]
struct MessageWithObjectConfig {
    secret: String,
    host: Option<String>,
    config: ObjectConfig,
}

enum Message {
    StringConfig(MessageWithStringConfig),
    ObjectConfig(MessageWithObjectConfig),
}

impl Message {
    fn parse(msg: &str) -> DecodeResult<Message> {
        let obj_msg: DecodeResult<MessageWithObjectConfig> = json::decode(msg);
        let string_msg: DecodeResult<MessageWithStringConfig> = json::decode(msg);
        match (obj_msg, string_msg) {
            (Ok(m), _) => Ok(Message::ObjectConfig(m)),
            (_, Ok(m)) => Ok(Message::StringConfig(m)),
            (Err(e1), Err(_)) => Err(e1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Message,
                MessageWithStringConfig,
                MessageWithObjectConfig};
    use rustc_serialize::json;
    use rustc_serialize::json::DecodeResult;

    #[test]
    fn test_basic_message() {
        let msg = r#"{
            "secret": "shh",
            "host": "127.0.0.1",
            "config": {
                "greeting": "sup",
                "farewell": "l8r"
            }
        }"#;


        let cmd = Message::parse(msg).unwrap();

        // let cmd: MessageWithObjectConfig = json::decode(msg).unwrap();
        assert_eq!("shh", cmd.secret());
        assert_eq!("127.0.0.1", cmd.host().unwrap());

        // let config = cmd.config;
        // let greeting = config.get("greeting").unwrap();
        // let farewell = config.get("farewell").unwrap();
        // assert_eq!("sup", greeting.as_slice());
        // assert_eq!("l8r", farewell.as_slice());
    }
}
