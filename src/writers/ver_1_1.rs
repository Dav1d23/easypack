/*!
# Packer 1.1 version.

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
- u64 (8 bytes) position in the file
- u64 (8 bytes) size of the content
- u8 (1 byte) size of the related name of the content
- as many bytes as specified above for the name of the content

* FOOTER

- u64 (4 bytes) the position of the `ToC` table in the file
- u64 (4 bytes) the number of records

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

#[derive(Debug)]
struct TocEntry {
    record_name: String,
    data_start: u64,
    data_len: u64,
}

impl TocEntry {
    const fn new(record_name: String, data_start: u64, data_len: u64) -> Self {
        Self {
            record_name,
            data_start,
            data_len,
        }
    }

    fn same_record_name(&self, other: &str) -> bool {
        self.record_name == other
    }

    // @TODO: This clippy report seems wrong, report?
    #[allow(clippy::missing_const_for_fn)]
    fn extract(self) -> (u64, u64, String) {
        (self.data_start, self.data_len, self.record_name)
    }
}

/**
The `Packer`, implemented as an easy state machine to prevent API misuse.

# Usage.

- create the packer using `from_writer`;
- write the headers using `write_header`;
- write each record using `write_record`;
- write the `ToC` and the footer using `close`.

If `close` is not called, the Packer will panic when dropped because the
written file would be inconsistent.
*/
pub struct Packer<S: Steps, W: Write> {
    // This is the writing position. It is needed to know where we are in the
    // file.
    pos: u64,
    // Note: behind an option to make the Drop check happy.
    writer: Option<W>,
    _step: PhantomData<S>,
    // The TableOfContent (`ToC`), filled in when a record is written.
    // Note: behind an option to make the Drop check happy.
    toc: Option<Vec<TocEntry>>,
}

impl<W: Write> Packer<NoneStep, W> {
    #[must_use]
    /// Create a Packer, writing data using the given writer.
    pub const fn from_writer(writer: W) -> Packer<HeaderStep, W> {
        Packer {
            pos: 0,
            writer: Some(writer),
            _step: PhantomData,
            toc: Some(vec![]),
        }
    }
}

impl<W: Write> Packer<HeaderStep, W> {
    /// Write the header of the file.
    /// The `ToC` is located at the bottom of the file, and we want to keep it
    /// like that since this allow us to expand the file without the need to
    /// rewrite it completely - we can just load the old `ToC`, change it / add
    /// new records, and write it back at the end. The old table will be
    /// ignored this way, but there won't be anything else other than some disk
    /// space lost.
    /// # Errors
    /// Any IO error.
    pub fn write_header(&mut self) -> Result<Packer<RecordStep, W>> {
        write_header(&mut self.writer.as_mut().expect("Writer is expected to be Some since the only way to construct the Packer is via `from_writer`"))?;
        Ok(Packer {
            pos: self.pos + utils::HEADER_SIZE,
            writer: self.writer.take(),
            _step: PhantomData,
            toc: self.toc.take(),
        })
    }
}

impl<W: Write> Packer<RecordStep, W> {
    /// Write a single record.
    /// This function internally update the `ToC`, that is written with the
    /// `close` call.
    /// # Errors
    /// In case the record's name is invalid, or the same as another already
    /// inserted record.
    pub fn write_record(&mut self, record: utils::Record) -> Result<()> {
        let data_start = self.pos;
        let data_len: u64 = record.data.len() as u64;
        let data_end = self.pos + data_len;

        write_record(
            &mut self.writer.as_mut().expect(
                "Writer is Some, since otherwise we should have panicked when writing the headers.",
            ),
            &record.data,
        )?;

        if self
            .toc
            .as_ref()
            .expect("ToC is Some here, we built it in the Header step.")
            .iter()
            .any(|r| r.same_record_name(&record.name))
        {
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
        self.toc
            .as_mut()
            .expect("ToC is Some here, we built it in the Header step.")
            .push(TocEntry::new(record.name, data_start, data_len));
        self.pos = data_end;
        Ok(())
    }

    /// Write the toc, the footer, and consume the Packer.
    /// # Errors
    /// Any IO error.
    pub fn close(mut self) -> Result<()> {
        let table_pos = self.pos;
        let mut how_many: u64 = 0;

        // This loop consumes the toc entries and write all of them.
        for entry in &mut self
            .toc
            .take()
            .expect("ToC is Some here, we built it in the Header step.")
            .into_iter()
        {
            let written_data = write_toc_entry(
                &mut self
                    .writer
                    .as_mut()
                    .expect("Writer is Some here, by construction."),
                entry,
            )?;
            let written_data: u64 = written_data.try_into()?;
            // Note: pos is updated, even tho it is not used anymore after
            // this. Let's call it "cleanness".
            self.pos += written_data;
            how_many += 1;
        }

        // Then the last bytes tells where to find the toc in the file itself.
        self.writer
            .as_mut()
            .expect("Writer is Some here, by construction.")
            .write_all(&table_pos.to_le_bytes())?;
        self.writer
            .as_mut()
            .expect("Writer is Some here, by construction.")
            .write_all(&how_many.to_le_bytes())?;

        Ok(())
    }
}

impl<S: Steps, W: Write> Drop for Packer<S, W> {
    /// Check if the `ToC` has been written. If not, panic.
    fn drop(&mut self) {
        if let Some(toc) = self.toc.as_ref() {
            assert!(toc.is_empty(), "Packer is dropped, but the `Table of Contents` has not been flushed. Perhaps you need to call `close`?");
        }
    }
}

pub fn write_header<W: Write>(w: &mut W) -> Result<()> {
    w.write_all(utils::FILE_TYPE.as_bytes())?;
    // Write version.
    w.write_all(&1u8.to_le_bytes())?;
    w.write_all(&1u8.to_le_bytes())?;
    Ok(())
}

fn write_record<W: Write>(w: &mut W, data: &[u8]) -> Result<()> {
    w.write_all(data)?;
    Ok(())
}

/// `Toc` contains the position in the file, the length of the string as u64,
/// the length of the string to be read, and and the bytes of the string
/// itself.
/// This function returns the amount of bytes being written.
fn write_toc_entry<W: Write>(w: &mut W, toc_entry: TocEntry) -> Result<usize> {
    let (pos, size, name) = toc_entry.extract();
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
    // This is the amount of bytes this function is writing.
    Ok(std::mem::size_of::<u64>()
        + std::mem::size_of::<u64>()
        + std::mem::size_of::<u8>()
        + name.len())
}
