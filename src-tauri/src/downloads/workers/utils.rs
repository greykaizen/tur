pub fn exponential_backoff(retry: u8, base_delay_ms: u32) -> u64 {
    (base_delay_ms as u64) * 2u64.pow(retry.saturating_sub(1) as u32)
}

pub fn preallocate_file(path: &str, size: usize) -> std::io::Result<()> {
    let file = std::fs::File::create(path)?;
    file.set_len(size as u64)?;
    // No zeroing needed if FS supports sparse files or we trust set_len
    Ok(())
}
