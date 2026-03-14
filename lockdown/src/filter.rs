use tracing::{info, warn};

/// Marker comments used to identify our entries in the hosts file.
const MARKER_START: &str = "# >>> LOCKDOWN MANAGED - DO NOT EDIT BELOW >>>";
const MARKER_END: &str = "# <<< LOCKDOWN MANAGED - DO NOT EDIT ABOVE <<<";

/// Returns the hosts file path for the current OS.
fn hosts_path() -> &'static str {
    if cfg!(windows) {
        r"C:\Windows\System32\drivers\etc\hosts"
    } else {
        "/etc/hosts"
    }
}

/// Read the current hosts file content. Returns an error description on failure.
fn read_hosts() -> Result<String, String> {
    std::fs::read_to_string(hosts_path())
        .map_err(|e| format!("Failed to read hosts file at {}: {e}", hosts_path()))
}

/// Write content to the hosts file. Returns an error description on failure.
fn write_hosts(content: &str) -> Result<(), String> {
    std::fs::write(hosts_path(), content)
        .map_err(|e| format!("Failed to write hosts file at {}: {e}", hosts_path()))
}

/// Remove all lockdown-managed entries from the hosts file content.
fn strip_managed_section(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut inside_managed = false;

    for line in content.lines() {
        if line.trim() == MARKER_START {
            inside_managed = true;
            continue;
        }
        if line.trim() == MARKER_END {
            inside_managed = false;
            continue;
        }
        if !inside_managed {
            result.push_str(line);
            result.push('\n');
        }
    }

    // Remove trailing blank lines we may have introduced.
    while result.ends_with("\n\n") {
        result.pop();
    }

    result
}

/// Apply the given list of blocked domains to the hosts file.
///
/// Each domain gets an entry pointing to `127.0.0.1` (and `::1` for IPv6).
/// We also block `www.` variants automatically.
pub fn apply_blocks(domains: &[String]) -> Result<(), String> {
    let existing = read_hosts()?;
    let mut clean = strip_managed_section(&existing);

    if domains.is_empty() {
        write_hosts(&clean)?;
        info!("Cleared all managed hosts entries");
        return Ok(());
    }

    clean.push('\n');
    clean.push_str(MARKER_START);
    clean.push('\n');

    for domain in domains {
        let domain = domain.trim().to_lowercase();
        if domain.is_empty() {
            continue;
        }

        // Validate: basic domain sanity check.
        if domain.contains(' ') || domain.contains('/') {
            warn!("Skipping invalid domain: {domain}");
            continue;
        }

        clean.push_str(&format!("127.0.0.1 {domain}\n"));
        clean.push_str(&format!("::1 {domain}\n"));

        // Also block www variant if not already a www domain.
        if !domain.starts_with("www.") {
            clean.push_str(&format!("127.0.0.1 www.{domain}\n"));
            clean.push_str(&format!("::1 www.{domain}\n"));
        }
    }

    clean.push_str(MARKER_END);
    clean.push('\n');

    write_hosts(&clean)?;
    info!("Applied {} domain blocks to hosts file", domains.len());
    Ok(())
}

/// Remove all lockdown-managed entries from the hosts file.
pub fn clear_blocks() -> Result<(), String> {
    let existing = read_hosts()?;
    let clean = strip_managed_section(&existing);
    write_hosts(&clean)?;
    info!("Cleared all managed hosts entries");
    Ok(())
}
