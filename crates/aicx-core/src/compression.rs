use std::io::{Cursor, Read};

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
            ArchiveProfile::Fast => vec![
                CodecKind::None,
                CodecKind::Lz4,
                CodecKind::Zstd,
                CodecKind::Gzip,
            ],
            ArchiveProfile::Balanced => vec![
                CodecKind::None,
                CodecKind::Zstd,
                CodecKind::Lz4,
                CodecKind::Gzip,
            ],
            ArchiveProfile::Max => vec![
                CodecKind::None,
                CodecKind::Zstd,
                CodecKind::Gzip,
                CodecKind::Lz4,
                CodecKind::Xz,
            ],
            ArchiveProfile::QrMax => vec![
                CodecKind::None,
                CodecKind::Zstd,
                CodecKind::Xz,
                CodecKind::Gzip,
                CodecKind::Lz4,
            ],
            ArchiveProfile::Agent => vec![
                CodecKind::None,
                CodecKind::Zstd,
                CodecKind::Lz4,
                CodecKind::Gzip,
            ],
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

fn read_limited<R: Read>(mut reader: R, max_output_size: usize, codec_name: &str) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut chunk = [0u8; 8192];

    loop {
        let read = reader.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        let next_len = output
            .len()
            .checked_add(read)
            .ok_or_else(|| AicxError::Validation(format!("{codec_name} output size overflow")))?;
        if next_len > max_output_size {
            return Err(AicxError::Validation(format!(
                "{codec_name} decompression exceeds manifest chunk size"
            )));
        }
        output.extend_from_slice(&chunk[..read]);
    }

    Ok(output)
}

pub fn decompress(codec: CodecKind, data: &[u8], max_output_size: usize) -> Result<Vec<u8>> {
    Ok(match codec {
        CodecKind::None => {
            if data.len() > max_output_size {
                return Err(AicxError::Validation(
                    "uncompressed chunk exceeds manifest chunk size".to_string(),
                ));
            }
            data.to_vec()
        }
        CodecKind::Zstd => {
            let decoder = zstd::stream::read::Decoder::new(Cursor::new(data))?;
            read_limited(decoder, max_output_size, "zstd")?
        }
        CodecKind::Lz4 => {
            if data.len() < 4 {
                return Err(AicxError::Validation(
                    "lz4 block is missing size prefix".to_string(),
                ));
            }
            let expected_size = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
            if expected_size > max_output_size {
                return Err(AicxError::Validation(
                    "lz4 decompression exceeds manifest chunk size".to_string(),
                ));
            }
            lz4_flex::block::decompress_size_prepended(data)
                .map_err(|err| AicxError::Validation(format!("lz4 decode failed: {err}")))?
        }
        CodecKind::Gzip => {
            use flate2::read::GzDecoder;
            let mut decoder = GzDecoder::new(Cursor::new(data));
            read_limited(&mut decoder, max_output_size, "gzip")?
        }
        CodecKind::Xz => {
            use xz2::read::XzDecoder;
            let mut decoder = XzDecoder::new(Cursor::new(data));
            read_limited(&mut decoder, max_output_size, "xz")?
        }
    })
}

pub fn select_codec(
    profile: ArchiveProfile,
    kind: FileKind,
    data: &[u8],
) -> Result<(CodecKind, i32, Vec<u8>)> {
    let mut candidates = candidate_codecs(profile, kind).into_iter();
    let first_codec = candidates
        .next()
        .ok_or_else(|| AicxError::Validation("no candidate codecs available".to_string()))?;
    let mut best_codec = first_codec;
    let mut best_level = codec_level(first_codec, profile);
    let mut best_data = compress(first_codec, best_level, data)?;

    for codec in candidates {
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

#[cfg(test)]
mod tests {
    use super::{compress, decompress};
    use crate::model::CodecKind;

    #[test]
    fn gzip_decompression_respects_max_output_size() {
        let payload = b"hello world";
        let compressed = compress(CodecKind::Gzip, 6, payload).expect("compress gzip");
        let error = decompress(CodecKind::Gzip, &compressed, payload.len() - 1)
            .expect_err("expected size limit error");
        assert!(error
            .to_string()
            .contains("gzip decompression exceeds manifest chunk size"));
    }
}
