/*!
# Packer 1.0 version.

All numbers are written in little endian format.
The structure of the packed file is as following:

* HEADER

- 4 bytes magic number
- 1 byte for the major version
- 1 byte for the minor version

* RECORDS

A list of records. the location in the file and the size to read is specified
in the `ToC`

* TOC (Table of Contents)

A list of
- u32 (4 bytes) position in the file
- u32 (4 bytes) size of the content
- u8 (1 byte) size of the related name of the content
- as many bytes as specified above for the name of the content

* FOOTER

- u32 (4 bytes) the position of the `ToC` table in the file
- u32 (4 bytes) the number of records

*/

use std::io::Write;
use std::marker::PhantomData;

use crate::error::{EasypackError, Result};
use crate::utils;

pub trait Steps {}

macro_rules! writersteps {
    ($name: tt) => {
        pub struct $name {}
        impl Steps for $name {}
    };
}

writersteps!(NoneStep);
writersteps!(HeaderStep);
writersteps!(RecordStep);

/// The packer, which can be used to pack data to the given writer.
pub struct Packer<S: Steps, W: Write> {
    pos: u32,
    writer: W,
    _step: PhantomData<S>,
    toc: Vec<(u32, u32, String)>,
}

impl<W: Write> Packer<NoneStep, W> {
    #[must_use]
    /// Create a Packer, using the given writer.
    pub fn from_writer(writer: W) -> Packer<HeaderStep, W> {
        Packer {
            pos: 0,
            writer,
            _step: PhantomData,
            toc: vec![],
        }
    }
}

impl<W: Write> Packer<HeaderStep, W> {
    /// Write the header of the file.
    /// # Errors
    /// Any IO error.
    pub fn write_header(mut self) -> Result<Packer<RecordStep, W>> {
        write_header(&mut self.writer)?;
        let val: u32 = utils::HEADER_SIZE.try_into()?;
        Ok(Packer {
            pos: self.pos + val,
            writer: self.writer,
            _step: PhantomData,
            toc: self.toc,
        })
    }
}

impl<W: Write> Packer<RecordStep, W> {
    /// Write a single record.
    /// # Errors
    /// In case the record's name is invalid, or the same as another already
    /// inserted record.
    pub fn write_record(&mut self, record: utils::Record) -> Result<()> {
        let data_start = self.pos;
        let data_len: u32 = record.data.len().try_into()?;
        let data_end = self.pos + data_len;

        write_record(&mut self.writer, &record.data)?;

        if self.toc.iter().any(|r| r.2 == record.name) {
            return Err(EasypackError::RecordSameName(format!(
                "Name {} has already been used.",
                record.name
            )));
        }
        if record.name.len() > u8::MAX.into() {
            return Err(EasypackError::RecordNameTooBig(
                "Unable to write a record with name len > u8::MAX bytes.".into(),
            ));
        }
        self.toc.push((data_start, data_len, record.name));
        self.pos = data_end;
        Ok(())
    }

    /// Write the toc, the footer, and consume the Packer.
    /// # Errors
    /// Any IO error.
    pub fn close(mut self) -> Result<()> {
        let table_pos = self.pos;
        let mut how_many: u32 = 0;

        for entry in &mut self.toc.drain(..) {
            write_toc_entry(&mut self.writer, &entry)?;
            how_many += 1;
        }

        self.writer.write_all(&table_pos.to_le_bytes())?;
        self.writer.write_all(&how_many.to_le_bytes())?;

        Ok(())
    }
}

pub fn write_header<W: Write>(w: &mut W) -> Result<()> {
    w.write_all(utils::FILE_TYPE.as_bytes())?;
    // Write version.
    w.write_all(&1u8.to_le_bytes())?;
    w.write_all(&0u8.to_le_bytes())?;
    Ok(())
}

fn write_record<W: Write>(w: &mut W, data: &[u8]) -> Result<()> {
    w.write_all(data)?;
    Ok(())
}

// Toc contains the position in the file, the length of the string as u32, the
// length of the string to be read, and and the bytes of the string itself.
pub fn write_toc_entry<W: Write>(w: &mut W, toc_entry: &(u32, u32, String)) -> Result<()> {
    let (pos, size, name) = toc_entry;
    w.write_all(&pos.to_le_bytes())?;
    w.write_all(&size.to_le_bytes())?;
    if name.len() > u8::MAX.into() {
        return Err(EasypackError::RecordNameTooBig(format!(
            "Record name is too big: len is {}, while only names up to {} are allowed",
            name.len(),
            u8::MAX
        )));
    }
    #[allow(clippy::pedantic)]
    // Checked above about this condition.
    w.write_all(&(name.len() as u8).to_le_bytes())?;
    w.write_all(&name.as_bytes()[..name.len()])?;
    Ok(())
}
