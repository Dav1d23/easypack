use std::io::{Read, Seek};

use crate::error::{EasypackError, Result};
use crate::utils;

pub mod ver_1_0;
pub mod ver_1_1;

/// The internal trait that defines an unpacker.
/// Every unpacker is related to a different version, that can be completely
/// different from every others. That's why we try to keep the unpackers free
/// to behave as they want.
pub trait VersionedUnpacker<'r> {
    /// Initialize the unpacker, if needed.
    /// # Errors
    /// In case the initialization fails.
    fn init(&mut self) -> Result<()>;
    /// Read the record associated with `record_name`, if any.
    /// # Errors
    /// In case the record name is too long.
    fn read_record(&mut self, record_name: &str) -> Result<Option<utils::Record>>;
}

/// Read the header, and get the version out (maj, min)
pub fn read_header<R: Read + Seek>(r: &mut R) -> Result<utils::Version> {
    r.rewind()?;
    let mut buf = vec![0; 4];
    if r.read(&mut buf[..4])? != 4 {
        return Err(EasypackError::InvalidFileError(
            "Not enough bytes in the header".to_owned(),
        ));
    }
    let header = String::from_utf8(buf[..4].to_vec())
        .map_err(|e| EasypackError::InvalidFileError(format!("Unable to read the header: {e}")))?;
    if header.as_bytes() != utils::FILE_TYPE.as_bytes() {
        return Err(EasypackError::InvalidFileError(format!(
            "Header does not match, found {header}"
        )));
    }
    if r.read(&mut buf[..2])? != 2 {
        return Err(EasypackError::InvalidFileError(
            "Not enough bytes in the version".to_owned(),
        ));
    }
    // Unwrap is ok here since I'm checking that I have 2 values above.
    #[allow(clippy::get_first)]
    let v1 = [*buf.get(0).unwrap(); 1];
    let v2 = [*buf.get(1).unwrap(); 1];
    let v1 = u8::from_le_bytes(v1);
    let v2 = u8::from_le_bytes(v2);
    let version = utils::Version::from((v1, v2));
    Ok(version)
}

/// Read the version from the header, if possible.
pub fn get_unpacker<'r, R: Read + Seek>(
    r: &'r mut R,
) -> Result<Box<dyn VersionedUnpacker<'r> + 'r>> {
    let version = read_header(r)?;

    match version.into() {
        (1, 0) => Ok(Box::new(ver_1_0::Unpacker::from_reader(r))),
        (1, 1) => Ok(Box::new(ver_1_1::Unpacker::from_reader(r))),
        el => Err(EasypackError::InvalidFileError(format!(
            "Found version `{el:?}`, which is not supported."
        ))),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::io::{BufReader, BufWriter, Cursor, Write};

    #[test]
    fn version_from_text() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut buff = Cursor::new(vec![]);
        {
            let mut w = BufWriter::new(&mut buff);
            w.write_all(b"This is just text.")?;
        }
        {
            let mut r = BufReader::new(&mut buff);
            assert!(read_header(&mut r).is_err());
        }
        Ok(())
    }

    #[test]
    fn get_unpacker_version_1_0() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut buff = Cursor::new(vec![]);
        {
            let mut w = BufWriter::new(&mut buff);
            w.write_all(b"SMPL")?;
            w.write_all(&1_u8.to_le_bytes())?;
            w.write_all(&0_u8.to_le_bytes())?;
        }
        {
            let mut r = BufReader::new(&mut buff);
            let unpacker = get_unpacker(&mut r);
            assert!(unpacker.is_ok());
        }
        Ok(())
    }

    #[test]
    fn get_unpacker_version_1_1() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut buff = Cursor::new(vec![]);
        {
            let mut w = BufWriter::new(&mut buff);
            w.write_all(b"SMPL")?;
            w.write_all(&1_u8.to_le_bytes())?;
            w.write_all(&1_u8.to_le_bytes())?;
        }
        {
            let mut r = BufReader::new(&mut buff);
            let unpacker = get_unpacker(&mut r);
            assert!(unpacker.is_ok());
        }
        Ok(())
    }

    #[test]
    fn unkown_version() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut buff = Cursor::new(vec![]);
        {
            let mut w = BufWriter::new(&mut buff);
            w.write_all(b"SMPL")?;
            w.write_all(&0_u8.to_le_bytes())?;
            w.write_all(&12_u8.to_le_bytes())?;
        }
        {
            let mut r = BufReader::new(&mut buff);
            let version = read_header(&mut r)?;
            assert_eq!(version, (0, 12).into());
        }
        Ok(())
    }

    #[test]
    fn wrong_header() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut buff = Cursor::new(vec![]);
        {
            let mut w = BufWriter::new(&mut buff);
            w.write_all(b"ASDF")?;
            w.write_all(&1_u8.to_le_bytes())?;
            w.write_all(&0_u8.to_le_bytes())?;
        }
        {
            let mut r = BufReader::new(&mut buff);
            assert_eq!(
                read_header(&mut r).unwrap_err().to_string(),
                "InvalidFileError(\"Header does not match, found ASDF\")".to_owned()
            );
        }
        Ok(())
    }
}
