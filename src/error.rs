#[derive(Debug)]
pub struct Error {
    pub desc: &'static str,
    pub subject: Option<String>,
}
