//! Binární formát souboru `.pidt`: magic + deflate(postcard(Timetable)).

use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;

use super::Timetable;

const MAGIC: &[u8] = b"PIDT2";

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("soubor není jízdní řád PID Trackeru")]
    BadMagic,
    #[error("poškozená komprese: {0}")]
    Inflate(#[from] std::io::Error),
    #[error("neplatná data: {0}")]
    Decode(#[from] postcard::Error),
}

pub fn encode(t: &Timetable) -> Vec<u8> {
    let raw = postcard::to_allocvec(t).expect("serializace Timetable nemůže selhat");
    let mut out = MAGIC.to_vec();
    let mut enc = DeflateEncoder::new(&mut out, Compression::best());
    enc.write_all(&raw).expect("zápis do Vec nemůže selhat");
    enc.finish().expect("zápis do Vec nemůže selhat");
    out
}

pub fn decode(bytes: &[u8]) -> Result<Timetable, CodecError> {
    let body = bytes.strip_prefix(MAGIC).ok_or(CodecError::BadMagic)?;
    let mut raw = Vec::new();
    DeflateDecoder::new(body).read_to_end(&mut raw)?;
    Ok(postcard::from_bytes(&raw)?)
}
