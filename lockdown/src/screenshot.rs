#[cfg(windows)]
use tracing::info;

/// Capture the primary monitor and return PNG bytes.
///
/// On Windows, this shells out to PowerShell using .NET's
/// System.Drawing and System.Windows.Forms to do a screen capture.
/// No external crates needed — just the built-in .NET framework.
pub fn capture_screen() -> Result<Vec<u8>, String> {
    #[cfg(windows)]
    {
        capture_screen_windows()
    }

    #[cfg(not(windows))]
    {
        Err("Screenshot capture is only supported on Windows".into())
    }
}

#[cfg(windows)]
fn capture_screen_windows() -> Result<Vec<u8>, String> {
    use std::process::Command;

    // Create a temp file path for the screenshot.
    let temp_dir = std::env::temp_dir();
    let screenshot_path = temp_dir.join("lockdown_screenshot.png");
    let path_str = screenshot_path
        .to_str()
        .ok_or_else(|| "Invalid temp path".to_string())?;

    // PowerShell script that captures the entire virtual screen.
    let ps_script = format!(
        r#"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bitmap = New-Object System.Drawing.Bitmap($screen.Width, $screen.Height)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($screen.Left, $screen.Top, 0, 0, $screen.Size)
$graphics.Dispose()
$bitmap.Save('{}')
$bitmap.Dispose()
"#,
        path_str.replace('\\', "\\\\").replace('\'', "''")
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
        .output()
        .map_err(|e| format!("Failed to run PowerShell: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("PowerShell screenshot failed: {stderr}"));
    }

    let png_bytes = std::fs::read(&screenshot_path)
        .map_err(|e| format!("Failed to read screenshot file: {e}"))?;

    // Clean up temp file (best effort).
    let _ = std::fs::remove_file(&screenshot_path);

    info!("Screenshot captured: {} bytes", png_bytes.len());
    Ok(png_bytes)
}
