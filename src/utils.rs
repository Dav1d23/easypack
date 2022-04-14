/// The file header.
pub static FILE_TYPE: &str = "SMPL";
/// The header size.
pub static HEADER_SIZE: u64 = 6;

#[derive(Debug, PartialEq)]
pub struct Version {
    maj: u8,
    min: u8,
}

impl From<(u8, u8)> for Version {
    fn from(v: (u8, u8)) -> Self {
        Self { maj: v.0, min: v.1 }
    }
}

impl From<Version> for (u8, u8) {
    fn from(v: Version) -> Self {
        (v.maj, v.min)
    }
}

/// The abstraction over a single record in the file.
pub struct Record {
    pub name: String,
    pub data: Vec<u8>,
}

impl Record {
    #[must_use]
    /// Create a new record.
    pub fn new(name: String, data: Vec<u8>) -> Self {
        Self { name, data }
    }
}

#[cfg(test)]
pub mod test {
    use std::fs;
    use std::ops::Deref;
    use std::path::PathBuf;

    pub struct Tempfile {
        path: PathBuf,
    }

    impl Tempfile {
        pub fn from_path(path: PathBuf) -> Self {
            assert!(!path.exists(), "Please remove {:?}", &path);
            Self { path }
        }
    }

    impl Deref for Tempfile {
        type Target = PathBuf;
        fn deref(&self) -> &Self::Target {
            &self.path
        }
    }

    impl Drop for Tempfile {
        fn drop(&mut self) {
            if self.path.exists() && self.path.is_file() {
                fs::remove_file(&self.path)
                    .unwrap_or_else(|e| eprintln!("Unable to remove `{:?}`: {}", &self.path, e));
            }
        }
    }
}
