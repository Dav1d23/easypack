#![warn(clippy::nursery)]
#![warn(clippy::pedantic)]

/*!
# Easypack: a simple, no-dependencies data packer/unpacker.

This crate provides an easy way to pack multiple files in a single one.
It can be useful to pack multiple read-only data in a single binary file,
for instance in case there are multiple small binary files that we need to
read together. Moreover, this is more convenient that asking the OS to open
a lot of small files, as overheads is reduced in here.

The main API is quite simple: one can create (pack) multiple data/files in one,
and read (unpack) them when needed. Note that while a Packer structure is
exposed, the related Unpacker is not since we internally allow multiple
versions to work. Thus, the API guarantees to always write the latest version,
and unpack all the supported versions.


# Pack and unpdack data from file.

In the following example, we can see how we can pack data in a single file, and retrieve some.

```
use easypack::*;
# use std::path::PathBuf;
# use std::str::FromStr;

# let packed_data_file = PathBuf::from_str("/tmp/__test_doc__.bin").unwrap();
// Pack some `Record`s into the given file
pack_records(
    &packed_data_file,
    [
        Record::new("c1".into(), vec![0x12, 0x34]),
        Record::new("c2".into(), vec![0x34]),
    ]
    .into_iter(),
).unwrap();

// Now unpack and assert on the content. Read the API for the return value of
// `unpack_records`.
let res = unpack_records(&packed_data_file, ["c2", "nope"].into_iter()).unwrap();
assert_eq!(res.0.len(), 1);
assert_eq!(res.1.len(), 1);

let record = res.0.get(0).unwrap();
assert_eq!(record.data.as_slice(), vec![0x34]);

let name_not_found = res.1.get(0).unwrap();
assert_eq!(name_not_found, "nope");
# std::fs::remove_file(&packed_data_file).unwrap();
```

The API also provides a way to pack and unpack directly from/to file.

```
use easypack::*;
# use std::path::PathBuf;
# use std::str::FromStr;
# use std::fs::OpenOptions;
# use std::io::Write;
# use std::io::Read;

# let packed_data_file = PathBuf::from_str("/tmp/__example_2_docstring.bin").unwrap();
# let content_1 = PathBuf::from_str("/tmp/__content_1.txt").unwrap();
# let content_2 = PathBuf::from_str("/tmp/__content_2.txt").unwrap();
# // Put some content in the 2 input files
# {
#     let mut f = OpenOptions::new()
#         .create(true)
#         .write(true)
#         .open(&*content_1).unwrap();
#     f.write_all(b"some bytes from content_1").unwrap();
#     f.flush().unwrap();
# }
# {
#     let mut f = OpenOptions::new()
#         .create(true)
#         .write(true)
#         .open(&*content_2).unwrap();
#     f.write_all(b"some other bytes from content_2").unwrap();
#     f.flush().unwrap();
# }

// Pack 2 files using `pack_files`, and associate a name with the files.
pack_files(
    &packed_data_file,
    [("c1", &content_1), ("c2", &content_2)].into_iter(),
).unwrap();

// ...
# let dumped = PathBuf::from_str("/tmp/__content_2_unpacked.txt").unwrap();
# assert!(!dumped.exists());

// Now unpack and assert on the content. Check `unpack_files` signature for
// details on the returned value.
let res = unpack_files(
    &packed_data_file,
    [("c1", &dumped)].into_iter(),
).unwrap();
assert_eq!(res.len(), 0);
# assert!(dumped.exists());

// Check that there is some content as well.
// ...
# let mut dumped_content = vec![];
# {
#     let mut dumped_file = std::fs::File::open(&dumped).unwrap();
#     dumped_file.read_to_end(&mut dumped_content).unwrap();
# }
# let dumped_content = String::from_utf8(dumped_content).unwrap();
assert_eq!(dumped_content, "some bytes from content_1".to_owned());

# std::fs::remove_file(&packed_data_file).unwrap();
# std::fs::remove_file(&content_1).unwrap();
# std::fs::remove_file(&content_2).unwrap();
# std::fs::remove_file(&dumped).unwrap();
*/

use std::fs::OpenOptions;
use std::io::{BufReader, BufWriter};
use std::io::{Read, Write};
use std::path::Path;

mod error;
mod readers;
mod utils;
mod writers;

use crate::error::Result;
pub use crate::utils::Record;
pub use crate::writers::Packer;

/// Pack the given `records` in the specified `outfile`.
///
/// # Errors
///
/// Check `EasyPackError` for the possible errors.
pub fn pack_records(
    outfile: impl AsRef<Path>,
    records: impl Iterator<Item = Record>,
) -> Result<()> {
    let outfile = OpenOptions::new().create(true).write(true).open(&outfile)?;
    let bufwriter = BufWriter::new(outfile);

    let mut writer = Packer::from_writer(bufwriter).write_header()?;
    for record in records {
        writer.write_record(record)?;
    }
    writer.close()?;
    Ok(())
}

/// Pack the given `records` in the specified `outfile`, which already contains
/// packed data. This operation is effectively an update.
///
/// # Errors
///
/// Check `EasyPackError` for the possible errors.
pub fn pack_records_update(
    outfile: impl AsRef<Path>,
    records: impl Iterator<Item = Record>,
) -> Result<()> {
    let (old_toc, file_size, version) = {
        let infile = OpenOptions::new().create(false).read(true).open(&outfile)?;
        let file_size = infile
            .metadata()
            .expect("Unable to read the size of the file")
            .len();

        let mut bufreader = BufReader::new(infile);
        let version = readers::read_header(&mut bufreader)?;
        let mut unpacker = readers::get_unpacker(&mut bufreader)?;
        // Init the unpacker, otherwise the Toc is empty
        // XXX Bad, should do something to avoid the need to "remember" about
        // this detail :)
        unpacker.init()?;

        let mut old_toc = vec![];
        unpacker
            .inspect_toc(&mut |pos, size, name| {
                old_toc.push((*pos, *size, name.clone()));
            })
            .expect("Unable to read the toc?");
        (old_toc, file_size, version)
    };
    let initial_toc: Vec<_> = old_toc
        .into_iter()
        .map(|(pos, size, name)| writers::TocEntry::new(name, pos, size))
        .collect();
    {
        let outfile = OpenOptions::new()
            .create(false)
            .append(true)
            .open(&outfile)?;
        let bufwriter = BufWriter::new(outfile);
        let mut packer = Packer::from_writer(bufwriter);
        let mut writer = packer.append_mode(initial_toc, file_size, &version)?;
        for record in records {
            writer.write_record(record)?;
        }
        writer.close()?;
    }

    Ok(())
}

/// Pack the given `files` in the specified `outfile`.
///
/// # Errors
///
/// Check `EasyPackError` for the possible errors.
pub fn pack_files<P: AsRef<Path>, T: AsRef<str>>(
    outfile: P,
    pack_from: impl Iterator<Item = (T, P)>,
) -> Result<()> {
    let outfile = OpenOptions::new().create(true).write(true).open(&outfile)?;
    let bufwriter = BufWriter::new(outfile);

    let mut writer = Packer::from_writer(bufwriter).write_header()?;
    for (record_name, path) in pack_from {
        let mut file = OpenOptions::new().read(true).open(&path)?;
        let mut data = vec![];
        let _howmany = file.read_to_end(&mut data)?;
        let record = Record::new(record_name.as_ref().to_owned(), data);
        writer.write_record(record)?;
    }
    writer.close()?;
    Ok(())
}

/// Unpack a set of records associated with the `names` in the `infile`.
///
/// # Returns
///
/// A tuple with the records that were found, and the names of these that we
/// did not find.
///
/// # Errors
///
/// Check `EasyPackError` for the possible errors.
pub fn unpack_records<T: AsRef<str>>(
    infile: impl AsRef<Path>,
    names: impl Iterator<Item = T>,
) -> Result<(Vec<utils::Record>, Vec<String>)> {
    let infile = OpenOptions::new().create(false).read(true).open(&infile)?;
    let mut bufreader = BufReader::new(infile);

    let mut unpacker = readers::get_unpacker(&mut bufreader)?;
    unpacker.init()?;
    let mut found = vec![];
    let mut notfound = vec![];
    for name in names {
        let nameref = name.as_ref();
        let record = unpacker.read_record(nameref)?;
        record.map_or_else(
            || {
                notfound.push(nameref.to_owned());
            },
            |record| {
                found.push(record);
            },
        );
    }
    Ok((found, notfound))
}

/// Unpack data from `infile`.
/// The user has to provide a slice of tuples(record name, output file).
///
/// # Returns
///
/// The list of records that were not found in the file.
///
/// # Errors
///
/// Check `EasyPackError` for the possible errors.
pub fn unpack_files<T: AsRef<str>, P: AsRef<Path>>(
    infile: P,
    unpack_to: impl Iterator<Item = (T, P)>,
) -> Result<Vec<String>> {
    let infile = OpenOptions::new().create(false).read(true).open(&infile)?;
    let mut bufreader = BufReader::new(infile);

    let mut unpacker = readers::get_unpacker(&mut bufreader)?;
    unpacker.init()?;
    let mut res = vec![];
    for (record_name, outpath) in unpack_to {
        if let Some(record) = unpacker.read_record(record_name.as_ref())? {
            let mut outfile = OpenOptions::new().create(true).write(true).open(outpath)?;
            outfile.write_all(&record.data)?;
        } else {
            res.push(record_name.as_ref().to_owned());
        }
    }
    Ok(res)
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::readers::VersionedUnpacker;
    use crate::utils::test::Tempfile;
    use crate::{readers, writers};

    use predicates::prelude::*;
    use std::io::{BufReader, BufWriter, Cursor};
    use std::path::PathBuf;
    use std::str::FromStr;

    #[test]
    /// Test that we can write a `ver_1_0` header, and read it.
    fn write_read_header_1_0() -> std::result::Result<(), Box<dyn std::error::Error>> {
        use crate::writers::ver_1_0::write_header;

        let mut buff = Cursor::new(vec![]);
        {
            let mut w = BufWriter::new(&mut buff);
            write_header(&mut w)?;
        }
        {
            let mut r = BufReader::new(&mut buff);
            let version = readers::read_header(&mut r)?;
            assert_eq!(version, (1, 0).into());
        }
        Ok(())
    }

    #[test]
    /// Test that we can write a `ver_1_1` header, and read it.
    fn write_read_header_1_1() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut buff = Cursor::new(vec![]);
        {
            let mut w = BufWriter::new(&mut buff);
            writers::write_header(&mut w)?;
        }
        {
            let mut r = BufReader::new(&mut buff);
            let version = readers::read_header(&mut r)?;
            assert_eq!(version, (1, 1).into());
        }
        Ok(())
    }

    #[test]
    /// Mixing versions should not work.
    fn read_mix_version() -> std::result::Result<(), Box<dyn std::error::Error>> {
        use crate::readers::ver_1_0::Unpacker;
        use crate::writers::ver_1_1::Packer;
        let mut buff = Cursor::new(vec![]);

        let buffwriter = BufWriter::new(&mut buff);
        let mut writer = Packer::from_writer(buffwriter).write_header()?;
        writer.write_record(utils::Record::new(
            "file_1".to_owned(),
            vec![0x12, 0x34, 0x56],
        ))?;
        writer.write_record(utils::Record::new(
            "file_2".to_owned(),
            vec![0x87, 0x65, 0x43],
        ))?;
        writer.close()?;

        let mut buffreader = BufReader::new(&mut buff);
        let mut reader = Unpacker::from_reader(&mut buffreader);
        reader.init()?;
        let r = reader.read_record("asd")?;
        assert!(r.is_none());
        let r = reader.read_record("file_1")?;
        // Since we are using a different writer, the reader is unable to find the file that is there!
        assert!(r.is_none());

        Ok(())
    }

    #[test]
    /// Mixing versions should not work, attempt number 2.
    fn read_mix_version_2() -> std::result::Result<(), Box<dyn std::error::Error>> {
        use crate::readers::ver_1_1::Unpacker;
        use crate::writers::ver_1_0::Packer;
        let mut buff = Cursor::new(vec![]);

        let buffwriter = BufWriter::new(&mut buff);
        let mut writer = Packer::from_writer(buffwriter).write_header()?;
        writer.write_record(utils::Record::new(
            "file_1".to_owned(),
            vec![0x12, 0x34, 0x56],
        ))?;
        writer.write_record(utils::Record::new(
            "file_2".to_owned(),
            vec![0x87, 0x65, 0x43],
        ))?;
        writer.close()?;

        let mut buffreader = BufReader::new(&mut buff);
        let mut reader = Unpacker::from_reader(&mut buffreader);
        // The reader needs more data to read, using a different reader does not work!
        assert!(reader.init().is_err());

        Ok(())
    }

    #[test]
    /// We can write and read records.
    fn read_write_records_1_0() -> std::result::Result<(), Box<dyn std::error::Error>> {
        use crate::readers::ver_1_0::Unpacker;
        use crate::writers::ver_1_0::Packer;
        let mut buff = Cursor::new(vec![]);

        let buffwriter = BufWriter::new(&mut buff);
        let mut writer = Packer::from_writer(buffwriter).write_header()?;
        writer.write_record(utils::Record::new(
            "file_1".to_owned(),
            vec![0x12, 0x34, 0x56],
        ))?;
        writer.write_record(utils::Record::new(
            "this_name_is_longer_than_24_chars_and_so_version_1_0_should_fail".to_owned(),
            vec![0x87, 0x65, 0x43],
        ))?;
        writer.close()?;

        let mut buffreader = BufReader::new(&mut buff);
        let mut reader = Unpacker::from_reader(&mut buffreader);
        reader.init()?;
        let r = reader.read_record("asd")?;
        assert!(r.is_none());
        let r = reader
            .read_record("this_name_is_longer_than_24_chars_and_so_version_1_0_should_fail")?;
        assert!(r.is_some());
        assert_eq!(r.unwrap().data, vec![0x87, 0x65, 0x43]);

        Ok(())
    }

    #[test]
    /// We can write and read records.
    fn read_write_records_1_1() -> std::result::Result<(), Box<dyn std::error::Error>> {
        use crate::readers::ver_1_1::Unpacker;
        use crate::writers::ver_1_1::Packer;

        let mut buff = Cursor::new(vec![]);

        let buffwriter = BufWriter::new(&mut buff);
        let mut writer = Packer::from_writer(buffwriter).write_header()?;
        writer.write_record(utils::Record::new(
            "file_1".to_owned(),
            vec![0x12, 0x34, 0x56],
        ))?;
        writer.write_record(utils::Record::new(
            "this_name_is_longer_than_24_chars_but__version_1_1_should_work_just_fine".to_owned(),
            vec![0x87, 0x65, 0x43],
        ))?;
        writer.close()?;

        let mut buffreader = BufReader::new(&mut buff);
        let mut reader = Unpacker::from_reader(&mut buffreader);
        reader.init()?;
        let r = reader.read_record("asd")?;
        assert!(r.is_none());
        let r = reader.read_record(
            "this_name_is_longer_than_24_chars_but__version_1_1_should_work_just_fine",
        )?;
        assert!(r.is_some());
        assert_eq!(r.unwrap().data, vec![0x87, 0x65, 0x43]);

        Ok(())
    }

    #[test]
    /// Complete test using files.
    fn pack_unpack_files() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let packed_file = Tempfile::from_path(PathBuf::from_str("/tmp/packedfile.bin")?);
        let infile1 = Tempfile::from_path(PathBuf::from_str("/tmp/afile.txt")?);
        let infile2 = Tempfile::from_path(PathBuf::from_str("/tmp/another.txt")?);

        // Prepare the 2 input files.
        {
            let mut f = OpenOptions::new()
                .create(true)
                .write(true)
                .open(&*infile1)?;
            f.write_all(b"This is some content!")?;
            f.flush()?;
        }
        {
            let mut f = OpenOptions::new()
                .create(true)
                .write(true)
                .open(&*infile2)?;
            f.write_all(b"something else")?;
            f.flush()?;
        }

        // Pack them.
        pack_files(
            &*packed_file,
            [("c1", &*infile1), ("c2", &*infile2)].into_iter(),
        )?;

        let outfile1 = Tempfile::from_path(PathBuf::from_str("/tmp/content_of_afile.txt")?);
        let outfile2 = Tempfile::from_path(PathBuf::from_str("/tmp/content_of_another.txt")?);

        // Now unpack and assert on the content.
        let res = unpack_files(
            &*packed_file,
            [("c2", &*outfile1), ("c1", &*outfile2)].into_iter(),
        )?;
        assert_eq!(res.len(), 0);

        let predicate_file = predicate::path::eq_file(&*outfile1).utf8().ok_or("none?")?;
        assert!(predicate_file.eval("something else"));
        let predicate_file = predicate::path::eq_file(&*outfile2).utf8().ok_or("none?")?;
        assert!(predicate_file.eval("This is some content!"));
        Ok(())
    }

    #[test]
    /// Complete test using files.
    fn pack_unpack_records() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let packed_file = Tempfile::from_path(PathBuf::from_str("/tmp/anotherpacked.bin")?);
        // Pack them.
        pack_records(
            &*packed_file,
            [
                utils::Record::new("c1".into(), vec![0x12, 0x34]),
                utils::Record::new("c2".into(), vec![0x34]),
            ]
            .into_iter(),
        )?;

        // Now unpack and assert on the content.
        let res = unpack_records(&*packed_file, ["c2", "c1", "nope"].into_iter())?;
        assert_eq!(res.0.len(), 2);
        assert_eq!(res.1.len(), 1);

        let record = res.0.get(0).unwrap();
        assert_eq!(record.data.as_slice(), vec![0x34]);

        let record = res.0.get(1).unwrap();
        assert_eq!(record.data.as_slice(), vec![0x12, 0x34]);

        let record = res.1.get(0).unwrap();
        assert_eq!(record, "nope");

        Ok(())
    }

    #[test]
    /// Update a file, without making a new one.
    fn update_file() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let packed_file = Tempfile::from_path(PathBuf::from_str("/tmp/anotherpacked_2.bin")?);
        {
            // Pack some data
            pack_records(
                &*packed_file,
                [
                    utils::Record::new("packed_first_1".into(), vec![0x12, 0x34]),
                    utils::Record::new("packed_first_2".into(), vec![0x34]),
                ]
                .into_iter(),
            )?;
        }

        {
            // Add some records
            pack_records_update(
                &*packed_file,
                [
                    utils::Record::new("repacked_1".into(), vec![0x67, 0x89]),
                    utils::Record::new("repacked_2".into(), vec![0x66]),
                ]
                .into_iter(),
            )?;
        }

        // Unpack and verify that all content is there.
        let res = unpack_records(
            &*packed_file,
            [
                "packed_first_1",
                "packed_first_2",
                "repacked_1",
                "repacked_2",
            ]
            .into_iter(),
        )?;

        dbg!(&res);
        assert_eq!(res.0.len(), 4);
        assert_eq!(res.1.len(), 0);

        assert_eq!(res.0.get(0).unwrap().data.as_slice(), [0x12, 0x34]);
        assert_eq!(res.0.get(1).unwrap().data.as_slice(), [0x34]);
        assert_eq!(res.0.get(2).unwrap().data.as_slice(), [0x67, 0x89]);
        assert_eq!(res.0.get(3).unwrap().data.as_slice(), [0x66]);

        std::mem::forget(packed_file);

        Ok(())
    }
}
