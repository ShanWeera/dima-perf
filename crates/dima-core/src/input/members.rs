//! Concatenated gzip members and bzip2 streams, read the way `gzip -d` and `bzip2 -d` read
//! them. Compressed files are often concatenations (`cat a.gz b.gz`, pbzip2, BGZF), so every
//! member is decoded in turn. After the last one, zero bytes are padding (tape and block
//! devices write them) and are ignored, as GNU gzip does; any other trailing bytes are reported
//! as such ([`TrailingData`]) instead of as a damaged header.

use std::io::{self, BufRead, Read};

/// Bytes after the end of the compressed data that are neither another member nor zero
/// padding.
#[derive(Debug)]
pub(crate) struct TrailingData {
    /// The compression's name.
    pub format: &'static str,
    /// How many bytes follow (at least one is not zero).
    pub bytes: u64,
}

impl std::fmt::Display for TrailingData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} bytes that are not {} data follow the end of the compressed data",
            self.bytes, self.format
        )
    }
}

impl std::error::Error for TrailingData {}

/// Decodes members one after another: `open` starts a decoder on the stored bytes, `close`
/// takes them back once the member ended.
pub(crate) struct Members<R, D> {
    decoder: Option<D>,
    open: fn(R) -> D,
    close: fn(D) -> R,
    magic: &'static [u8],
    format: &'static str,
}

impl<R: BufRead, D: Read> Members<R, D> {
    pub(crate) fn new(
        stored: R,
        open: fn(R) -> D,
        close: fn(D) -> R,
        magic: &'static [u8],
        format: &'static str,
    ) -> Self {
        Self {
            decoder: Some(open(stored)),
            open,
            close,
            magic,
            format,
        }
    }
}

impl<R: BufRead, D: Read> Read for Members<R, D> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            let Some(decoder) = self.decoder.as_mut() else {
                return Ok(0);
            };
            let n = decoder.read(buf)?;
            if n > 0 {
                return Ok(n);
            }
            // The member ended (and its checksum was verified).
            let Some(decoder) = self.decoder.take() else {
                return Ok(0);
            };
            let mut stored = (self.close)(decoder);
            if next_member(&mut stored, self.magic, self.format)? {
                self.decoder = Some((self.open)(stored));
            }
        }
    }
}

/// After a member: `true` when another starts, `false` at the end of the input or of its zero
/// padding. Bytes that start like the format's magic number are left to the decoder, which
/// reports a damaged header.
fn next_member(r: &mut impl BufRead, magic: &[u8], format: &'static str) -> io::Result<bool> {
    let buf = r.fill_buf()?;
    if buf.is_empty() {
        return Ok(false);
    }
    let shared = buf.len().min(magic.len());
    if buf[..shared] == magic[..shared] {
        return Ok(true);
    }
    let (mut bytes, mut padding) = (0u64, true);
    loop {
        let buf = r.fill_buf()?;
        if buf.is_empty() {
            break;
        }
        padding &= buf.iter().all(|&b| b == 0);
        bytes += buf.len() as u64;
        let n = buf.len();
        r.consume(n);
    }
    if padding {
        Ok(false)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            TrailingData { format, bytes },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn gzip(text: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        e.write_all(text).unwrap();
        e.finish().unwrap()
    }

    fn bzip2(text: &[u8]) -> Vec<u8> {
        let mut e = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::fast());
        e.write_all(text).unwrap();
        e.finish().unwrap()
    }

    fn read_gz(bytes: &[u8]) -> io::Result<Vec<u8>> {
        let mut out = Vec::new();
        Members::new(
            io::BufReader::with_capacity(7, bytes),
            flate2::bufread::GzDecoder::new,
            flate2::bufread::GzDecoder::into_inner,
            &[0x1F, 0x8B],
            "gzip",
        )
        .read_to_end(&mut out)?;
        Ok(out)
    }

    fn read_bz(bytes: &[u8]) -> io::Result<Vec<u8>> {
        let mut out = Vec::new();
        Members::new(
            io::BufReader::with_capacity(7, bytes),
            bzip2::bufread::BzDecoder::new,
            bzip2::bufread::BzDecoder::into_inner,
            b"BZh",
            "bzip2",
        )
        .read_to_end(&mut out)?;
        Ok(out)
    }

    #[test]
    fn concatenated_members_and_zero_padding() {
        let mut gz = [gzip(b">a\nAC\n"), gzip(b""), gzip(b">b\nGT\n")].concat();
        assert_eq!(read_gz(&gz).unwrap(), b">a\nAC\n>b\nGT\n");
        gz.extend([0u8; 1000]);
        assert_eq!(read_gz(&gz).unwrap(), b">a\nAC\n>b\nGT\n", "padding");
        let bz = [bzip2(b">a\nAC\n"), bzip2(b">b\nGT\n")].concat();
        assert_eq!(read_bz(&bz).unwrap(), b">a\nAC\n>b\nGT\n");
    }

    #[test]
    fn other_trailing_bytes_are_named() {
        for (bytes, format) in [
            ([gzip(b">a\nAC\n"), vec![0, 0, b'x', 0]].concat(), "gzip"),
            ([bzip2(b">a\nAC\n"), b"junk".to_vec()].concat(), "bzip2"),
        ] {
            let read = if format == "gzip" { read_gz } else { read_bz };
            let e = read(&bytes).unwrap_err();
            let t = e.get_ref().unwrap().downcast_ref::<TrailingData>().unwrap();
            assert_eq!((t.format, t.bytes), (format, 4));
        }
    }

    #[test]
    fn damaged_members_are_still_errors() {
        let gz = gzip(b">a\nACGTACGTACGT\n");
        assert!(read_gz(&gz[..gz.len() - 3]).is_err(), "truncated");
        // A second member with a damaged header.
        let bad = [gz.clone(), vec![0x1F, 0x8B, 0xFF, 0xFF, 0xFF]].concat();
        let e = read_gz(&bad).unwrap_err();
        assert!(e.get_ref().is_none_or(|i| !i.is::<TrailingData>()), "{e}");
    }
}
