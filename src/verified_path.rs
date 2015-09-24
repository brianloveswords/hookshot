use std::string::ToString;
use std::path::Path;
use std::fs;
use ::error::Error;

#[derive(Debug, Clone)]
pub struct VerifiedPath {
    path: String,
}

impl VerifiedPath {
    pub fn new(path: &str) -> Result<VerifiedPath, Error> {
        match file_exists(Path::new(&path)) {
            true => Ok(VerifiedPath { path: path.to_string() }),
            false => Err(Error {
                desc: "file doesn't exist",
                subject: Some(path.to_string()),
            }),
        }
    }
}
impl ToString for VerifiedPath {
    fn to_string(&self) -> String { self.path.clone()  }
}


fn file_exists(full_path: &Path) -> bool {
    match fs::metadata(full_path) {
        Err(_) => false,
        Ok(f) => f.is_file()
    }
}
