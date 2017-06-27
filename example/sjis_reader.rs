extern crate encoding_rs;
extern crate encoding_io;
use std::{env, fs};
use std::io::Read;
use encoding_rs::*;
use encoding_io::*;

fn main() {
    let mut args = env::args();
    let appname = args.next().unwrap();
    match args.next() {
        Some(path) => {
            let mut decoder = SHIFT_JIS.new_decoder();
            let mut reader = TextReader::new(fs::File::open(path).unwrap(), &mut decoder);
            let mut s = String::new();
            reader.read_to_string(&mut s).unwrap();
            println!("{}", s);
        },
        None => {
            println!("Usage: {} file-path", appname);
        }
    }
}
