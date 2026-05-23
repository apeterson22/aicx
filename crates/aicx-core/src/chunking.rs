pub fn fixed_size_chunks<'a>(data: &'a [u8], chunk_size: usize) -> Vec<&'a [u8]> {
    if chunk_size == 0 {
        return Vec::new();
    }
    data.chunks(chunk_size).collect()
}

