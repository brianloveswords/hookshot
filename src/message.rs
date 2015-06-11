use rustc_serialize::json;
use rustc_serialize::json::DecodeResult;
use std::collections::BTreeMap;
use std::string;

#[derive(RustcDecodable, Clone, Debug)]
pub struct RemoteCommand {
    pub secret: String,
    pub target: Option<String>,
    pub host: Option<String>,
    pub playbook: Option<String>,
}

#[derive(RustcDecodable, Debug)]
struct ObjectVars {
    config: BTreeMap<String, String>
}

#[derive(RustcDecodable, Debug)]
struct StringVars {
    config: string::String
}

/// Get the "extra" variables from a message. Currently they can come in
/// either as an object, which will be json encoded, or as a raw string
/// which will be passed directly to ansible as the -e parameter.
///
/// Example:
///   The following message
///   ```json
///   {
///     "secret": "shh",
///     "config": {
///       "var1": "Some value",
///       "var2": "Other value"
///     }
///   }
///   ```
///   would return `"{"var1":"Some value", "var2":"Other value"}"`.
///
///   Given a message where config is a string,
///   ```json
///   {
///     "secret": "shh",
///     "config": "var1='Some value' var2='Other value'"
///   }
///
///  it would return "var1='Some value' var2='Other value'".
///
pub fn get_extra_vars(msg: &str) -> DecodeResult<String> {
    let obj_msg: DecodeResult<ObjectVars> = json::decode(msg);
    let string_msg: DecodeResult<StringVars> = json::decode(msg);
    match (obj_msg, string_msg) {
        (Ok(m), _) => Ok(json::encode(&m.config).unwrap()),
        (_, Ok(m)) => Ok(m.config),
        // TODO: improve error handling: if both parse attempts fail, we
        // currently use the error from the parse into ObjectVar and
        // spit that back out when ideally we'd like a way to represent
        // both errors. Also we should have a special case for when
        // "config" is totally missing from the message so messages can
        // leave it out when it's not necessary to include extra
        // variables.
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
