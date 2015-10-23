use openssl::crypto::hash::Type as OpenSSLType;
use openssl::crypto::hmac::hmac;
use regex::Regex;
use std::fmt;
use std::string::ToString;

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum HashType {
    MD5,
    SHA1,
    SHA224,
    SHA256,
    SHA384,
    SHA512,
    RIPEMD160,
}
impl HashType {
    pub fn from_str(hasher: &str) -> Option<HashType> {
        match hasher {
            "md5" => Some(HashType::MD5),
            "sha1" => Some(HashType::SHA1),
            "sha224" => Some(HashType::SHA224),
            "sha256" => Some(HashType::SHA256),
            "sha384" => Some(HashType::SHA384),
            "sha512" => Some(HashType::SHA512),
            "ripemd160" => Some(HashType::RIPEMD160),
            _ => None,
        }
    }
    fn to_openssl(&self) -> OpenSSLType {
        match *self {
            HashType::MD5 => OpenSSLType::MD5,
            HashType::SHA1 => OpenSSLType::SHA1,
            HashType::SHA224 => OpenSSLType::SHA224,
            HashType::SHA256 => OpenSSLType::SHA256,
            HashType::SHA384 => OpenSSLType::SHA384,
            HashType::SHA512 => OpenSSLType::SHA512,
            HashType::RIPEMD160 => OpenSSLType::RIPEMD160,
        }

    }
}
impl ToString for HashType {
    fn to_string(&self) -> String {
        match *self {
            HashType::MD5 => String::from("md5"),
            HashType::SHA1 => String::from("sha1"),
            HashType::SHA224 => String::from("sha224"),
            HashType::SHA256 => String::from("sha256"),
            HashType::SHA384 => String::from("sha384"),
            HashType::SHA512 => String::from("sha512"),
            HashType::RIPEMD160 => String::from("ripemd160"),
        }
    }
}
fn bytes_to_hex(bytes: &Vec<u8>) -> String {
    let mut hex_string = String::new();
    for b in bytes.iter() {
        hex_string.push_str(&format!("{:0>2x}", b));
    }
    hex_string
}

#[derive(PartialEq, Debug, Clone)]
pub struct Signature {
    alg: HashType,
    hex: String,
}
impl Signature {
    pub fn from(sig: String) -> Option<Signature> {
        let re = Regex::new(r"^([:word:]+)=([:xdigit:]+)$").unwrap();

        let (hash, hex) = match re.captures(&sig) {
            None => return None,
            Some(caps) => match (caps.at(1), caps.at(2)) {
                (Some(hash), Some(hex)) => (hash, hex),
                _ => return None,
            },
        };

        if let Some(alg) = HashType::from_str(hash) {
            return Some(Signature {
                alg: alg,
                hex: String::from(hex),
            });
        }

        None
    }

    pub fn create(alg: HashType, data: &str, key: &str) -> Signature {
        let mac = hmac(alg.to_openssl(), key.as_bytes(), data.as_bytes());
        let hex = bytes_to_hex(&mac);
        Signature {
            alg: alg,
            hex: hex,
        }
    }

    pub fn verify(&self, data: &str, key: &str) -> bool {
        *self == Self::create(self.alg, data, key)
    }
}
impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}={}", self.alg.to_string(), self.hex)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equality() {
        // generated with `echo -n "data" | openssl dgst -sha1 -hmac "key"`
        let sigstring = String::from("sha1=104152c5bfdca07bc633eebd46199f0255c9f49d");
        let sig1 = Signature::create(HashType::SHA1, "data", "key");
        let sig2 = Signature::from(sigstring).unwrap();
        assert_eq!(sig1, sig2);
        assert!(sig1.verify("data", "key"));
    }
}
