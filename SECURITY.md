# Security Notes

AICX treats every archive as untrusted input.

- Validate header lengths and manifest tables before reading chunk data.
- Reject malformed offsets, lengths, duplicate output paths, and unsafe restore targets.
- Refuse symlink escapes and file overwrites unless explicitly enabled.
- Verify chunk and file hashes before writing restored output.

AICX does not perform encryption or execution. Those responsibilities belong to AegisQR in its separate repository.

