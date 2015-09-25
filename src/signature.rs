use regex::Regex;
use openssl::crypto::hash::Type as OpenSSLType;
use openssl::crypto::hmac::hmac;
use std::string::ToString;
use std::fmt;

#[derive(PartialEq, Debug)]
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
    pub fn from_str(hasher: &str) -> HashType {
        match hasher {
            "md5" => HashType::MD5,
            "sha1" => HashType::SHA1,
            "sha224" => HashType::SHA224,
            "sha256" => HashType::SHA256,
            "sha384" => HashType::SHA384,
            "sha512" => HashType::SHA512,
            "ripemd160" => HashType::RIPEMD160,
            _ => unimplemented!(),
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
            HashType::RIPEMD160 => OpenSSLType::RIPEMD160
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

#[derive(PartialEq, Debug)]
pub struct Signature {
    alg: HashType,
    hex: String,
}
impl Signature {
    pub fn from(sig: &str) -> Option<Signature> {
        let re = Regex::new(r"^([:word:]+)=([:xdigit:]+)$").unwrap();

        match re.captures(sig) {
            None => None,
            Some(caps) => {
                let hash_type = HashType::from_str(caps.at(1).unwrap());
                let hex_string = caps.at(2).unwrap();
                Some(Signature {alg: hash_type, hex: String::from(hex_string) })
            }
        }
    }

    pub fn create(alg: HashType, data: &str, key: &str) -> Signature {
        let mac = hmac(alg.to_openssl(), key.as_bytes(), data.as_bytes());
        let hex = bytes_to_hex(&mac);
        Signature { alg: alg, hex: hex }
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
        let sigstring = "sha1=104152c5bfdca07bc633eebd46199f0255c9f49d";
        let sig1 = Signature::create(HashType::SHA1, "data", "key");
        let sig2 = Signature::from(sigstring).unwrap();
        assert_eq!(sig1, sig2);
    }
}
