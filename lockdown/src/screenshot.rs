//! Screen capture using native Win32 BitBlt, encoded as PNG.
//!
//! Captures the primary monitor and compresses to PNG (~200-500KB)
//! instead of raw BMP (~4MB). No PowerShell, no external processes.

/// Encode BGRA pixels to PNG bytes.
#[allow(dead_code)]
fn encode_png(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    // Convert BGRA → RGBA in place (swap R and B channels).
    let mut rgba = pixels.to_vec();
    for chunk in rgba.chunks_exact_mut(4) {
        chunk.swap(0, 2); // B ↔ R
    }

    let mut buf = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut buf, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);

        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("PNG header error: {e}"))?;

        writer
            .write_image_data(&rgba)
            .map_err(|e| format!("PNG write error: {e}"))?;
    }

    Ok(buf)
}

/// Capture the primary screen and return PNG image bytes.
pub fn capture_screen() -> Result<Vec<u8>, String> {
    #[cfg(windows)]
    {
        let (pixels, w, h) = crate::win32::capture_screen()?;
        let png = encode_png(&pixels, w as u32, h as u32)?;
        tracing::info!("Screenshot captured: {}x{} ({} bytes PNG)", w, h, png.len());
        Ok(png)
    }

    #[cfg(not(windows))]
    {
        Err("Screenshot capture is only supported on Windows".into())
    }
}
