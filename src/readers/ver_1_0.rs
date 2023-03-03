//! The reader (unpacker) module.
//! It implements the basic functionalities to read data from a file.
use std::io::{Read, Seek, SeekFrom};

use crate::error::{EasypackError, Result};
use crate::utils;

/// The unpacker, which can be used to read data from the given reader.
pub struct Unpacker<'r, R: Read + Seek> {
    reader: &'r mut R,
    toc: Vec<(u32, u32, String)>,
}

impl<'r, R: Read + Seek> super::VersionedUnpacker<'r> for Unpacker<'r, R> {
    fn init(&mut self) -> Result<()> {
        self.read_toc()?;
        Ok(())
    }
    fn read_record(&mut self, record_name: &str) -> Result<Option<utils::Record>> {
        self.read_record(record_name)
    }
}

impl<'r, R: Read + Seek> Unpacker<'r, R> {
    #[must_use]
    /// Create an `Unpacker`, using the given writer.
    pub fn from_reader(reader: &'r mut R) -> Self {
        Self {
            reader,
            toc: vec![],
        }
    }

    /// Read the `ToC` from the file.
    /// # Errors
    /// In the input file is invalid.
    pub fn read_toc(&mut self) -> Result<()> {
        let (toc_position, toc_len) = read_footer(&mut self.reader)?;
        self.toc = read_toc_entries(&mut self.reader, toc_position, toc_len)?;

        Ok(())
    }

    /// Read a single record from the file, if there is some.
    /// # Errors
    /// In the input file is invalid.
    pub fn read_record(&mut self, name: &str) -> Result<Option<utils::Record>> {
        for (record_pos, record_len, record_name) in &self.toc {
            if name == record_name {
                #[allow(clippy::pedantic)]
                // I'm checking above that record_len is shorter than u32::MAX
                // if we are in a 32bit arch.
                let data = read_record(&mut self.reader, *record_pos, *record_len as usize)?;
                let rec = utils::Record::new(name.to_owned(), data);
                return Ok(Some(rec));
            }
        }

        Ok(None)
    }
}

pub fn read_footer<R: Read + Seek>(r: &mut R) -> Result<(u32, u32)> {
    r.seek(SeekFrom::End(-8))?;
    let mut buf = vec![0; 8];
    if r.read(&mut buf)? != 8 {
        return Err(EasypackError::InvalidFileError(
            "Not enough bytes in the footer".to_owned(),
        ));
    }
    let mut v = [0; 4];
    // Unwrap is ok, since I've already checked that we got 8 bytes.
    v.copy_from_slice(buf.get(..4).unwrap());
    let v1 = u32::from_le_bytes(v);
    // Unwrap is ok, since I've already checked that we got 8 bytes.
    v.copy_from_slice(buf.get(4..).unwrap());
    let v2 = u32::from_le_bytes(v);
    Ok((v1, v2))
}

pub fn read_record<R: Read + Seek>(r: &mut R, pos: u32, len: usize) -> Result<Vec<u8>> {
    r.seek(SeekFrom::Start(pos.into()))?;
    let mut res = Vec::with_capacity(len);
    #[allow(clippy::uninit_vec)]
    // Safety:
    // 1. I've set the capacity to str_len, so I've already enough space for this.
    // 2. I'm gonna override these bytes, so anything there is ok to be thrown away.
    unsafe {
        res.set_len(len);
    };
    let bytes_read = r.read(&mut res)?;
    if bytes_read != len {
        return Err(EasypackError::InvalidFileError(format!(
            "Not enough bytes to read: {bytes_read}",
        )));
    }

    Ok(res)
}

pub fn read_toc_entries<R: Read + Seek>(
    r: &mut R,
    toc_position: u32,
    how_many: u32,
) -> Result<Vec<(u32, u32, String)>> {
    const U32_SIZE: usize = std::mem::size_of::<u32>();

    r.seek(SeekFrom::Start(toc_position.into()))?;

    let mut res = vec![];

    for i in 0..how_many {
        let mut buf64 = [0_u8; U32_SIZE];
        let bytes_read = r.read(&mut buf64)?;
        if bytes_read != U32_SIZE {
            return Err(EasypackError::InvalidFileError(format!(
                "Not enough bytes to read the pos of the {i}th toc? bytes_read: {bytes_read}"
            )));
        }
        let pos = u32::from_le_bytes(buf64);

        let bytes_read = r.read(&mut buf64)?;
        if bytes_read != U32_SIZE {
            return Err(EasypackError::InvalidFileError(format!(
                "Not enough bytes to read the pos of the {i}th toc? bytes_read: {bytes_read}"
            )));
        }
        let size = u32::from_le_bytes(buf64);

        let mut buf8 = [0_u8; 1];
        let bytes_read = r.read(&mut buf8)?;
        if bytes_read != 1 {
            return Err(EasypackError::InvalidFileError(format!(
                "Not enough bytes to read the str_len of the {i}th toc? bytes_read: {bytes_read}"
            )));
        }
        let str_len = u8::from_le_bytes(buf8) as usize;

        let mut buf = Vec::with_capacity(str_len);
        #[allow(clippy::uninit_vec)]
        // Safety:
        // 1. I've set the capacity to str_len, so I've already enough space for this.
        // 2. I'm gonna override these bytes, so anything there is ok to be thrown away.
        unsafe {
            buf.set_len(str_len);
        };

        let bytes_read = r.read(&mut buf[..str_len])?;
        if bytes_read != str_len {
            return Err(EasypackError::InvalidFileError(format!(
                "Not enough bytes to read the name of the {i}th toc? bytes_read: {bytes_read}"
            )));
        }
        let name = String::from_utf8(buf).unwrap();

        res.push((pos, size, name));
    }
    Ok(res)
}
