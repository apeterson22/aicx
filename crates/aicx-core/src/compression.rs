use std::io::Cursor;

use crate::error::{AicxError, Result};
use crate::model::{ArchiveProfile, CodecKind, FileKind};

pub fn codec_level(codec: CodecKind, profile: ArchiveProfile) -> i32 {
    match (codec, profile) {
        (CodecKind::None, _) => 0,
        (CodecKind::Lz4, ArchiveProfile::Fast) => 0,
        (CodecKind::Lz4, _) => 1,
        (CodecKind::Gzip, ArchiveProfile::Fast) => 1,
        (CodecKind::Gzip, ArchiveProfile::Balanced) => 6,
        (CodecKind::Gzip, ArchiveProfile::Max) => 9,
        (CodecKind::Gzip, ArchiveProfile::QrMax) => 9,
        (CodecKind::Gzip, ArchiveProfile::Agent) => 6,
        (CodecKind::Gzip, ArchiveProfile::Secure) => 6,
        (CodecKind::Zstd, ArchiveProfile::Fast) => 1,
        (CodecKind::Zstd, ArchiveProfile::Balanced) => 3,
        (CodecKind::Zstd, ArchiveProfile::Max) => 9,
        (CodecKind::Zstd, ArchiveProfile::QrMax) => 12,
        (CodecKind::Zstd, ArchiveProfile::Agent) => 4,
        (CodecKind::Zstd, ArchiveProfile::Secure) => 4,
        (CodecKind::Xz, ArchiveProfile::Fast) => 3,
        (CodecKind::Xz, ArchiveProfile::Balanced) => 6,
        (CodecKind::Xz, ArchiveProfile::Max) => 9,
        (CodecKind::Xz, ArchiveProfile::QrMax) => 9,
        (CodecKind::Xz, ArchiveProfile::Agent) => 6,
        (CodecKind::Xz, ArchiveProfile::Secure) => 6,
    }
}

fn candidate_codecs(profile: ArchiveProfile, kind: FileKind) -> Vec<CodecKind> {
    match kind {
        FileKind::Binary | FileKind::Archive | FileKind::Media => vec![CodecKind::None],
        _ => match profile {
            ArchiveProfile::Fast => vec![CodecKind::None, CodecKind::Lz4, CodecKind::Zstd, CodecKind::Gzip],
            ArchiveProfile::Balanced => vec![CodecKind::None, CodecKind::Zstd, CodecKind::Lz4, CodecKind::Gzip],
            ArchiveProfile::Max => vec![CodecKind::None, CodecKind::Zstd, CodecKind::Gzip, CodecKind::Lz4, CodecKind::Xz],
            ArchiveProfile::QrMax => vec![CodecKind::None, CodecKind::Zstd, CodecKind::Xz, CodecKind::Gzip, CodecKind::Lz4],
            ArchiveProfile::Agent => vec![CodecKind::None, CodecKind::Zstd, CodecKind::Lz4, CodecKind::Gzip],
            ArchiveProfile::Secure => vec![CodecKind::None, CodecKind::Zstd, CodecKind::Gzip],
        },
    }
}

pub fn compress(codec: CodecKind, level: i32, data: &[u8]) -> Result<Vec<u8>> {
    Ok(match codec {
        CodecKind::None => data.to_vec(),
        CodecKind::Zstd => zstd::stream::encode_all(Cursor::new(data), level)?,
        CodecKind::Lz4 => lz4_flex::block::compress_prepend_size(data),
        CodecKind::Gzip => {
            use flate2::{write::GzEncoder, Compression};
            let mut encoder = GzEncoder::new(Vec::new(), Compression::new(level.max(0) as u32));
            std::io::Write::write_all(&mut encoder, data)?;
            encoder.finish()?
        }
        CodecKind::Xz => {
            use std::io::Write;
            use xz2::write::XzEncoder;
            let mut encoder = XzEncoder::new(Vec::new(), level.max(0) as u32);
            encoder.write_all(data)?;
            encoder.finish()?
        }
    })
}

pub fn decompress(codec: CodecKind, data: &[u8]) -> Result<Vec<u8>> {
    Ok(match codec {
        CodecKind::None => data.to_vec(),
        CodecKind::Zstd => zstd::stream::decode_all(Cursor::new(data))?,
        CodecKind::Lz4 => lz4_flex::block::decompress_size_prepended(data)
            .map_err(|err| AicxError::Validation(format!("lz4 decode failed: {err}")))?,
        CodecKind::Gzip => {
            use flate2::read::GzDecoder;
            use std::io::Read;
            let mut decoder = GzDecoder::new(Cursor::new(data));
            let mut output = Vec::new();
            decoder.read_to_end(&mut output)?;
            output
        }
        CodecKind::Xz => {
            use std::io::Read;
            use xz2::read::XzDecoder;
            let mut decoder = XzDecoder::new(Cursor::new(data));
            let mut output = Vec::new();
            decoder.read_to_end(&mut output)?;
            output
        }
    })
}

pub fn select_codec(profile: ArchiveProfile, kind: FileKind, data: &[u8]) -> Result<(CodecKind, i32, Vec<u8>)> {
    let mut best_codec = CodecKind::None;
    let mut best_level = 0;
    let mut best_data = data.to_vec();

    for codec in candidate_codecs(profile, kind) {
        let level = codec_level(codec, profile);
        let compressed = compress(codec, level, data)?;
        if compressed.len() < best_data.len() {
            best_codec = codec;
            best_level = level;
            best_data = compressed;
        }
    }

    Ok((best_codec, best_level, best_data))
}
