use tracing::info;

// Raw FFI bindings for Windows APIs. These are only compiled on Windows.
#[cfg(windows)]
extern "system" {
    /// Locks the workstation display (Win32 API, user32.dll).
    fn LockWorkStation() -> i32;
    /// Blocks or unblocks keyboard and mouse input (Win32 API, user32.dll).
    /// Requires the calling process to be on the active input desktop
    /// and have the appropriate privileges.
    fn BlockInput(fBlockIt: i32) -> i32;
}

/// Lock the Windows workstation (shows the Windows lock screen).
///
/// # Note
/// This uses `LockWorkStation()` which triggers the standard Windows lock.
/// The local user can unlock with their Windows password. For a scenario
/// where only the remote controller can unlock, see `block_input`.
pub fn lock_workstation() -> Result<(), String> {
    #[cfg(windows)]
    {
        let result = unsafe { LockWorkStation() };
        if result == 0 {
            return Err("LockWorkStation failed (is the process running in session 0?)".into());
        }
        info!("Workstation locked via LockWorkStation()");
        Ok(())
    }

    #[cfg(not(windows))]
    {
        info!("[stub] lock_workstation called on non-Windows OS");
        Ok(())
    }
}

/// Block all keyboard and mouse input system-wide.
///
/// # Safety concerns
/// - Requires the process to run with administrator privileges.
/// - Input is blocked for ALL applications including Task Manager.
/// - A watchdog timer or remote unlock mechanism is essential.
/// - Windows will automatically unblock input if the calling thread's
///   message loop stops (e.g. process crashes), which is a safety net.
///
/// Pass `true` to block, `false` to unblock.
pub fn set_input_blocked(blocked: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        let flag = if blocked { 1 } else { 0 };
        let result = unsafe { BlockInput(flag) };
        if result == 0 {
            return Err(format!(
                "BlockInput({}) failed — process may lack admin privileges",
                if blocked { "true" } else { "false" }
            ));
        }
        info!("Input blocking set to: {blocked}");
        Ok(())
    }

    #[cfg(not(windows))]
    {
        info!("[stub] set_input_blocked({blocked}) called on non-Windows OS");
        Ok(())
    }
}

/// Combined lock: block input and lock the workstation.
pub fn engage_full_lock() -> Result<(), String> {
    set_input_blocked(true)?;
    lock_workstation()?;
    info!("Full lock engaged (input blocked + workstation locked)");
    Ok(())
}

/// Release all locks: unblock input.
pub fn disengage_lock() -> Result<(), String> {
    set_input_blocked(false)?;
    info!("Lock disengaged (input unblocked)");
    Ok(())
}
