use std::path::Path;
use std::fs;
use ::error::Error;

#[derive(Debug, Clone)]
pub struct VerifiedPath {
    path: String,
    root: String,
}

impl VerifiedPath {
    pub fn file(root: &Path, path: &Path) -> Result<VerifiedPath, Error> {
        let path_as_string = String::from(path.to_str().unwrap());
        let root_as_string = String::from(root.to_str().unwrap());
        match file_exists(&root.join(path)) {
            true => Ok(VerifiedPath {
                path: path_as_string,
                root: root_as_string,
            }),
            false => Err(Error {
                desc: "file doesn't exist",
                subject: Some(path_as_string),
            }),
        }
    }
    pub fn path(&self) -> String { self.path.clone()  }
    pub fn root(&self) -> String { self.root.clone()  }
}

pub fn file_exists(full_path: &Path) -> bool {
    match fs::metadata(full_path) {
        Err(_) => false,
        Ok(f) => f.is_file()
    }
}
