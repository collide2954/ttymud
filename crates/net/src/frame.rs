use std::io::{Read, Write};

use crate::Error;

pub(crate) fn write(writer: &mut impl Write, bytes: &[u8]) -> Result<(), Error> {
    let length = u16::try_from(bytes.len()).map_err(|_| Error::TooLarge)?;
    let mut frame = Vec::with_capacity(bytes.len() + 2);
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(bytes);
    writer.write_all(&frame)?;
    writer.flush()?;
    Ok(())
}

pub(crate) fn read(reader: &mut impl Read) -> Result<Vec<u8>, Error> {
    let mut header = [0; 2];
    reader.read_exact(&mut header)?;
    let mut bytes = vec![0; usize::from(u16::from_be_bytes(header))];
    reader.read_exact(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{read, write};
    use crate::Error;

    #[test]
    fn frames_round_trip() {
        for bytes in [Vec::new(), b"hello".to_vec(), vec![7; 65535]] {
            let mut stream = Vec::new();
            write(&mut stream, &bytes).unwrap();
            assert_eq!(stream.len(), bytes.len() + 2);
            assert_eq!(read(&mut Cursor::new(stream)).unwrap(), bytes);
        }
    }

    #[test]
    fn the_length_comes_first_in_big_endian() {
        let mut stream = Vec::new();
        write(&mut stream, &[9; 258]).unwrap();
        assert_eq!(stream[..2], [1, 2]);
    }

    #[test]
    fn frames_over_the_limit_fail() {
        let result = write(&mut Vec::new(), &vec![0; 65536]);
        assert!(matches!(result, Err(Error::TooLarge)));
    }

    #[test]
    fn an_early_end_means_a_closed_connection() {
        assert!(matches!(read(&mut Cursor::new(vec![])), Err(Error::Closed)));
        assert!(matches!(
            read(&mut Cursor::new(vec![0, 5, 1])),
            Err(Error::Closed)
        ));
    }
}
