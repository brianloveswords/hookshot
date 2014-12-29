use rustc_serialize::json;
use rustc_serialize::json::DecodeResult;
use std::collections::BTreeMap;
use std::string;

#[deriving(RustcDecodable, Clone, Show)]
pub struct RemoteCommand {
    pub secret: String,
    host: Option<String>,
    playbook: Option<String>,
}

#[deriving(RustcDecodable, Show)]
struct ObjectVars {
    config: BTreeMap<String, String>
}

#[deriving(RustcDecodable, Show)]
struct StringVars {
    config: string::String
}

pub fn get_extra_vars(msg: &str) -> DecodeResult<String> {
    let obj_msg: DecodeResult<ObjectVars> = json::decode(msg);
    let string_msg: DecodeResult<StringVars> = json::decode(msg);
    match (obj_msg, string_msg) {
        (Ok(m), _) => Ok(json::encode(&m.config)),
        (_, Ok(m)) => Ok(m.config),
        (Err(e), Err(_)) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::{RemoteCommand, get_extra_vars};
    use rustc_serialize::json;

    #[test]
    fn test_string_message() {
        let msg = r#"{
            "secret": "shh",
            "host": "127.0.0.1",
            "playbook": "deploy",
            "config": "var1=a var2=b"
        }"#;

        let command: RemoteCommand = json::decode(msg).unwrap();

        let expect = "var1=a var2=b";
        assert_eq!(expect, get_extra_vars(msg).unwrap());
        assert_eq!("shh", command.secret);
        assert_eq!("127.0.0.1", command.host.unwrap());
        assert_eq!("deploy", command.playbook.unwrap());
    }

    #[test]
    fn test_object_message() {
        let msg = r#"{
            "secret": "shh",
            "config": {"var1":"a"}
        }"#;

        let command: RemoteCommand = json::decode(msg).unwrap();

        let expect = "{\"var1\":\"a\"}";
        assert_eq!(expect, get_extra_vars(msg).unwrap());
        assert_eq!("shh", command.secret);
        assert_eq!(None, command.host);
        assert_eq!(None, command.playbook);
    }
}
