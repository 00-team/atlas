#[derive(Debug)]
#[allow(unused)]
pub enum AtlasError {
    BadJson(serde_json::Error),
    Io(std::io::Error),
    Osm(osmpbfreader::Error),
    IdAlreadyExists,
}

impl From<std::io::Error> for AtlasError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for AtlasError {
    fn from(value: serde_json::Error) -> Self {
        Self::BadJson(value)
    }
}

impl From<osmpbfreader::Error> for AtlasError {
    fn from(value: osmpbfreader::Error) -> Self {
        Self::Osm(value)
    }
}
