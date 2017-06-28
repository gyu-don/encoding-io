#![warn(missing_docs)]

extern crate encoding_rs;

use std::io::{self, BufRead, Read, Write};
use std::ptr;

use encoding_rs::*;

const BUF_CAPACITY: usize = 2048;

struct Buffer {
    inner: Box<[u8]>,
    pos: usize,
    cap: usize,
}
impl Buffer {
    fn with_capacity(capacity: usize) -> Buffer {
        let buf = vec![0; capacity].into_boxed_slice();
        Buffer { inner: buf, pos: 0, cap: 0 }
    }
    fn move_to_front(&mut self) {
        unsafe {
            if self.pos != 0 {
                ptr::copy(self.inner[self.pos..].as_ptr(),
                        self.inner.as_mut_ptr(),
                        self.cap - self.pos);
                self.cap -= self.pos;
                self.pos = 0;
            }
        }
    }
    fn read_from<'a, R: ?Sized>(&mut self, mut reader: &'a mut R) -> io::Result<usize>
            where &'a mut R: Read {
        let read_bytes = reader.read(&mut self.inner[self.cap..])?;
        self.cap += read_bytes;
        Ok(read_bytes)
    }
    fn write_into<'a, W: ?Sized>(&mut self, mut writer: &'a mut W) -> io::Result<usize>
            where &'a mut W: Write {
        let written_bytes = writer.write(&self.inner[self.pos..self.cap])?;
        self.pos += written_bytes;
        Ok(written_bytes)
    }
    fn get_mut_for_append(&mut self) -> &mut [u8] {
        &mut self.inner[self.cap..]
    }
    fn report_appended_bytes(&mut self, appended: usize) {
        self.cap += appended;
    }
    fn get_for_consume(&self) -> &[u8] {
        &self.inner[self.pos..self.cap]
    }
    fn report_consumed_bytes(&mut self, consumed: usize) {
        self.pos += consumed;
    }
    fn len(&self) -> usize {
        self.cap - self.pos
    }
}

pub struct TextReader<R> {
    inner: R,
    decoder: Decoder,
    unprocessed_buf: Buffer,
    decoded_buf: Buffer,
    finished: bool,
}

impl<'a, R: Read> TextReader<R> {
    pub fn new(reader: R, encoding: &'static Encoding) -> TextReader<R> {
        Self::with_capacity(reader, encoding, BUF_CAPACITY, BUF_CAPACITY)
    }
    pub fn with_capacity(reader: R, encoding: &'static Encoding,
                         unprocessed_buf_capacity: usize, decoded_buf_capacity: usize) -> TextReader<R> {
        TextReader {
            inner: reader,
            decoder: encoding.new_decoder(),
            unprocessed_buf: Buffer::with_capacity(unprocessed_buf_capacity),
            decoded_buf: Buffer::with_capacity(decoded_buf_capacity),
            finished: false,
        }
    }
    #[inline]
    fn decode_to_buf(&mut self) -> io::Result<()> {
        if !self.finished {
            self.unprocessed_buf.move_to_front();
            self.decoded_buf.move_to_front();
            let read_bytes = self.unprocessed_buf.read_from(&mut self.inner)?;
            let (result, consumed_bytes, written_bytes, _) = self.decoder.decode_to_utf8(
                    self.unprocessed_buf.get_for_consume(),
                    self.decoded_buf.get_mut_for_append(),
                    read_bytes == 0 || self.finished);
            match result {
                CoderResult::InputEmpty => {
                    if read_bytes == 0 { self.finished = true; }
                },
                CoderResult::OutputFull => {
                }
            }
            self.unprocessed_buf.report_consumed_bytes(consumed_bytes);
            self.decoded_buf.report_appended_bytes(written_bytes);
        }
        Ok(())
    }
}

impl<'a, R: Read> Read for TextReader<R> {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        if dst.len() > self.decoded_buf.len() {
            self.decode_to_buf()?;
        }
        self.decoded_buf.write_into(dst)
    }
}

impl<'a, R: Read> BufRead for TextReader<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.decode_to_buf()?;
        Ok(self.decoded_buf.get_for_consume())
    }
    fn consume(&mut self, amt: usize) {
        self.decoded_buf.report_consumed_bytes(amt)
    }
}

#[cfg(test)]
mod tests {
    use encoding_rs::*;
    use super::*;
    #[test]
    fn sjis_test() {
        let s = "テストaてすと嗚呼ああああ";
        let (v, _, _) = SHIFT_JIS.encode(s);
        let v: Vec<u8> = v.into_owned();
        println!("{:?}", v);
        let mut decoded = Vec::new();
        let mut streamdec = TextReader::new(v.as_slice(), SHIFT_JIS);
        streamdec.read_to_end(&mut decoded).unwrap();
        println!("{:?}", s.as_bytes());
        println!("{:?}", decoded.as_slice());
        assert_eq!(s.as_bytes(), decoded.as_slice());
    }

    #[test]
    fn sjis_test2() {
        //let mut sjisenc = SHIFT_JIS.new_encoder();
        let s = "テストaてすと嗚呼ああああ";
        let (v, _, _) = SHIFT_JIS.encode(s);
        let v: Vec<u8> = v.into_owned();
        println!("{:?}", v);
        let mut decoded = Vec::new();
        let mut streamdec = TextReader::with_capacity(v.as_slice(), SHIFT_JIS, 10, 10);
        streamdec.read_to_end(&mut decoded).unwrap();
        println!("{:?}", s.as_bytes());
        println!("{:?}", decoded.as_slice());
        assert_eq!(s.as_bytes(), decoded.as_slice());
    }
    #[test]
    fn lines_test() {
        let s = "\
あああおいあああ
ざあああ
っっっっががｋｌｋｆじゃちあ
afdalfいあｊｄふぃえ";
        let (v, _, _) = EUC_JP.encode(s);
        let v: Vec<u8> = v.into_owned();
        let reader = TextReader::with_capacity(v.as_slice(), EUC_JP, 5, 5);
        let mut lines = reader.lines();
        for line1 in s.lines() {
            let line2 = lines.next();
            assert!(line2.is_some());
            let line2 = line2.unwrap();
            assert!(line2.is_ok());
            let line2 = line2.unwrap();
            assert_eq!(line1, &line2);
        }
        assert!(lines.next().is_none());
    }
}
