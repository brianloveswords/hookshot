use std::path::Path;
use std::fs;
use ::error::Error;

#[derive(Debug, Clone)]
pub struct VerifiedPath {
    path: String,
}

// TODO: refactor this, it's basically 80% copypasta
impl VerifiedPath {
    pub fn file(root: Option<&Path>, path: &Path) -> Result<VerifiedPath, Error> {
        let path_as_string = String::from(path.to_str().unwrap());
        let full_path = match root {
            Some(root) => root.join(path),
            None => path.to_path_buf()
        };
        match file_exists(&full_path) {
            true => Ok(VerifiedPath { path: path_as_string, }),
            false => Err(Error {
                desc: "file doesn't exist",
                subject: Some(path_as_string),
            }),
        }
    }
    pub fn directory(root: Option<&Path>, path: &Path) -> Result<VerifiedPath, Error> {
        let path_as_string = String::from(path.to_str().unwrap());
        let full_path = match root {
            Some(root) => root.join(path),
            None => path.to_path_buf()
        };
        match directory_exists(&full_path) {
            true => Ok(VerifiedPath { path: path_as_string, }),
            false => Err(Error {
                desc: "file doesn't exist",
                subject: Some(path_as_string),
            }),
        }
    }
    pub fn path(&self) -> &Path { Path::new(&self.path) }
}

pub fn file_exists(full_path: &Path) -> bool {
    match fs::metadata(full_path) {
        Err(_) => false,
        Ok(f) => f.is_file()
    }
}
pub fn directory_exists(full_path: &Path) -> bool {
    match fs::metadata(full_path) {
        Err(_) => false,
        Ok(f) => f.is_dir()
    }
}
