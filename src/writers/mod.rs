#[allow(unused)]
pub mod ver_1_0;
pub mod ver_1_1;

/// version 11 is the default one;
pub use ver_1_1::*;

#[cfg(test)]
mod test {
    use super::*;
    use crate::utils;

    use std::io::{BufWriter, Cursor};

    #[test]
    /// We can't use the same record name twice.
    fn read_write_same_record_name_twice() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut buff = Cursor::new(vec![]);

        let buffwriter = BufWriter::new(&mut buff);
        let mut writer = Packer::from_writer(buffwriter).write_header()?;
        writer.write_record(utils::Record::new(
            "name".to_owned(),
            vec![0x12, 0x34, 0x56],
        ))?;

        // Assert using the same name twice is not ok.
        assert!(writer
            .write_record(utils::Record::new(
                "name".to_owned(),
                vec![0x87, 0x65, 0x43],
            ))
            .is_err());
        writer.close()?;
        Ok(())
    }

    #[test]
    /// We must use a "short" record name.
    fn record_name_too_long() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut buff = Cursor::new(vec![]);

        let buffwriter = BufWriter::new(&mut buff);
        let mut writer = Packer::from_writer(buffwriter).write_header()?;

        // This record's name is 255 char long.
        let res = writer
            .write_record(utils::Record::new(
                "This name is longer than the allowed u8::MAX bytes, but why would anyone name a file like that. qwertyuiopasdfghjklzxcvbnmqwertyuiopasdfghjklzxcv bnmqwertyuiopasdfghjklzxcvbnmqwertyuiopasdfghjklzxcvbnmqwertyuiopasdfghjklzxcvbnmqwertyuiopasdfghjklzxcvb 255".to_owned(),
                vec![0x12, 0x34, 0x56],
            ));
        assert!(res.is_ok());

        // And this is 256!
        let res = writer
            .write_record(utils::Record::new(
                "This name is longer than the allowed u8::MAX bytes, but why would anyone name a file like that. qwertyuiopasdfghjklzxcvbnmqwertyuiopasdfghjklzxcv bnmqwertyuiopasdfghjklzxcvbnmqwertyuiopasdfghjklzxcvbnmqwertyuiopasdfghjklzxcvbnmqwertyuiopasdfghjklzxcvbn 256".to_owned(),
                vec![0x12, 0x34, 0x56],
            ));
        assert!(res.is_err());
        writer.close()?;
        Ok(())
    }
}
