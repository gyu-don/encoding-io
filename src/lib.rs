#![deny(missing_docs)]

//! This crate is for handling non-utf8 text file on Rust.
//! `TextReader` is a reader which decodes non-utf8 text to utf8 text.
//! `TextWriter` is a writer which encodes utf8 text to non-utf8 text.

extern crate encoding_rs;

use std::io::{self, BufRead, Read, Write};
use std::{ptr, str};

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
    fn write_to<'a, W: ?Sized>(&mut self, mut writer: &'a mut W) -> io::Result<usize>
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
    fn report_consumed_all(&mut self) {
        self.pos = 0;
        self.cap = 0;
    }
    fn len(&self) -> usize {
        self.cap - self.pos
    }
}

/// `TextReader` is a wrapper for a non-utf8 encoded text reader.
/// It behaves as a utf8 encoded text reader.
///
/// Examples:
/// ```
/// use std::fs::File;
/// use std::io::{Read, BufRead};
/// use encoding_io::TextReader;
/// use encoding_rs::*;
/// let mut reader = TextReader::new(
///         File::open("shiftjis-text.txt").expect("failed to open."),
///         SHIFT_JIS);
/// for line in reader.lines() {
///     let line = line.expect("Failed to read.");
///     println!("{}", line);
/// }
/// ```
pub struct TextReader<R> {
    inner: R,
    decoder: Decoder,
    unprocessed_buf: Buffer,
    decoded_buf: Buffer,
    finished: bool,
}

impl<R: Read> TextReader<R> {
    /// Creates a new `TextReader` with a default buffer capacity.
    pub fn new(reader: R, encoding: &'static Encoding) -> TextReader<R> {
        Self::with_capacity(reader, encoding, BUF_CAPACITY, BUF_CAPACITY)
    }
    /// Creates a new `TextReader` with the specified buffer capacity.
    pub fn with_capacity(reader: R, encoding: &'static Encoding,
                         unprocessed_buf_capacity: usize, decoded_buf_capacity: usize) -> TextReader<R> {
        Self::with_decoder_and_capacity(reader, encoding.new_decoder(), unprocessed_buf_capacity, decoded_buf_capacity)
    }
    /// Creates a new `TextReader` with the specified decoder and a default buffer capacity.
    pub fn with_decoder(reader: R, decoder: Decoder) -> TextReader<R> {
        Self::with_decoder_and_capacity(reader, decoder, BUF_CAPACITY, BUF_CAPACITY)
    }
    /// Creates a new `TextReader` with the specified decoder and specified buffer capacity.
    pub fn with_decoder_and_capacity(reader: R, decoder: Decoder,
                         unprocessed_buf_capacity: usize, decoded_buf_capacity: usize) -> TextReader<R> {
        TextReader {
            inner: reader,
            decoder: decoder,
            unprocessed_buf: Buffer::with_capacity(unprocessed_buf_capacity),
            decoded_buf: Buffer::with_capacity(decoded_buf_capacity),
            finished: false,
        }
    }
    /// Consumes the `TextReader` and returns the underlying decoder.
    pub fn into_decoder(self) -> Decoder {
        self.decoder
    }
    /// Consumes the `TextReader` and returns the underlying reader.
    pub fn into_inner(self) -> R {
        self.inner
    }
    /// Consumes the `TextReader` and returns the underlying reader and decoder.
    pub fn into_decoder_and_inner(self) -> (Decoder, R) {
        (self.decoder, self.inner)
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

impl<R: Read> Read for TextReader<R> {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        if dst.len() > self.decoded_buf.len() {
            self.decode_to_buf()?;
        }
        self.decoded_buf.write_to(dst)
    }
}

impl<R: Read> BufRead for TextReader<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.decode_to_buf()?;
        Ok(self.decoded_buf.get_for_consume())
    }

    fn consume(&mut self, amt: usize) {
        self.decoded_buf.report_consumed_bytes(amt)
    }
}

/// `TextWriter` is a wrapper for writer.
/// It encodes given bytes to utf8 then write to a writer.
///
/// Examples
/// ```
/// use std::fs::File;
/// use std::io::Write;
/// use encoding_io::TextWriter;
/// use encoding_rs::*;
/// let mut writer = TextWriter::new(
///         File::create("shiftjis-text.txt").expect("Failed to create."),
///         SHIFT_JIS);
/// writer.write("some utf8 text".as_bytes()).expect("Failed to write.");
/// ```
pub struct TextWriter<W> {
    inner: W,
    encoder: Encoder,
    encoded_buf: Buffer,
}

impl<W: Write> TextWriter<W> {
    /// Creates a new `TextWriter` with a default buffer capacity.
    pub fn new(writer: W, encoding: &'static Encoding) -> TextWriter<W> {
        Self::with_capacity(writer, encoding, BUF_CAPACITY)
    }
    /// Creates a new `TextWriter` with the specified buffer capacity.
    pub fn with_capacity(writer: W, encoding: &'static Encoding, encoded_buf_capacity: usize) -> TextWriter<W> {
        Self::with_encoder_and_capacity(writer, encoding.new_encoder(), encoded_buf_capacity)
    }
    /// Creates a new `TextWriter` with the specified encoder.
    pub fn with_encoder(writer: W, encoder: Encoder) -> TextWriter<W> {
        Self::with_encoder_and_capacity(writer, encoder, BUF_CAPACITY)
    }
    /// Creates a new `TextWriter` with the specified encoder and specified buffer capacity.
    pub fn with_encoder_and_capacity(writer: W, encoder: Encoder, encoded_buf_capacity: usize) -> TextWriter<W> {
        TextWriter {
            inner: writer,
            encoder: encoder,
            encoded_buf: Buffer::with_capacity(encoded_buf_capacity),
        }
    }
    /// Consumes the `TextWriter` and returns the underlying encoder.
    pub fn into_encoder(self) -> Encoder {
        self.encoder
    }
    /// Consumes the `TextWriter` and returns the underlying writer.
    pub fn into_inner(self) -> W {
        self.inner
    }
    /// Consumes the `TextWriter` and returns the underlying writer and encoder.
    pub fn into_encoder_and_inner(self) -> (Encoder, W) {
        (self.encoder, self.inner)
    }
    /// Notice to encoder that the end of the stream is reached when all the characters in src have been consumed.
    /// This argument is needed for ISO-2022-JP and is ignored for other encodings.
    pub fn finish(&mut self) -> io::Result<()> {
        let empty = [];
        self._write(&empty, true).map(|_| ())
    }
    #[inline]
    fn _write(&mut self, buf: &[u8], last: bool) -> io::Result<usize> {
        let mut total_read = 0;
        loop {
            let (result, read_bytes, written_bytes, _) = unsafe {
                self.encoder.encode_from_utf8(str::from_utf8_unchecked(&buf[total_read..]), self.encoded_buf.get_mut_for_append(), last)
            };
            self.encoded_buf.report_appended_bytes(written_bytes);
            self.encoded_buf.write_to(&mut self.inner)?;
            total_read += read_bytes;
            match result {
                CoderResult::InputEmpty => {
                    return Ok(total_read);
                },
                CoderResult::OutputFull => {}
            }
            self.encoded_buf.move_to_front();
        }
    }
}

impl<W: Write> Write for TextWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self._write(buf, false)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.encoded_buf.len() > 1 {
            self.inner.write_all(self.encoded_buf.get_for_consume())?;
            self.encoded_buf.report_consumed_all();
        }
        self.inner.flush()
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
    #[test]
    fn write_test() {
        let v = Vec::new();
        let s = "あｆｄかｊふぁいふぁｋｊｂｇはふぁいうｆじゃがっｊｆ";
        let (enc, _, _) = SHIFT_JIS.encode(s);
        let sjis_encoded = enc.into_owned();
        let mut writer = TextWriter::new(v, SHIFT_JIS);
        writer.write(s.as_bytes()).unwrap();

        let v = writer.into_inner();
        assert_eq!(&v, &sjis_encoded);
    }
}
