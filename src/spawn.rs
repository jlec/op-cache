use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::process::Command;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::daemon;
use crate::error::Error;

const SPAWN_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Ensure the daemon is running, starting it if needed.
///
/// Uses a lock file to serialize concurrent spawn attempts so that only one
/// process runs the is_running -> spawn sequence at a time.
pub fn ensure_daemon_running(config: &Config) -> Result<(), Error> {
    let lock_path = config.socket_path.with_extension("lock");

    // Ensure the parent directory exists (socket may live in a user cache dir
    // that hasn't been created yet — the daemon would create it on bind, but
    // we need it here first for the lock file).
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| Error::SpawnFailed(format!("failed to create socket directory: {}", e)))?;
    }

    // Open (and create if necessary) the lock file
    let lock_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| Error::SpawnFailed(format!("failed to open spawn lock: {}", e)))?;

    // Acquire an exclusive lock. Blocks if another process is currently
    // in the check-then-spawn critical section.
    let ret = unsafe { libc::flock(lock_file.as_raw_fd(), libc::LOCK_EX) };
    if ret != 0 {
        return Err(Error::SpawnFailed(format!(
            "failed to acquire spawn lock: {}",
            std::io::Error::last_os_error()
        )));
    }

    // Re-check now that we hold the lock: another process may have just
    // started the daemon while we were waiting.
    if daemon::is_running(config) {
        return Ok(()); // lock_file drops here, releasing the flock
    }

    spawn_daemon()?;
    // lock_file drops here, releasing the flock
    wait_for_daemon(config)
}

fn spawn_daemon() -> Result<(), Error> {
    let exe_path = std::env::current_exe()
        .map_err(|e| Error::SpawnFailed(format!("failed to get executable path: {}", e)))?;

    // Spawn the daemon process - it will fork and daemonize itself
    Command::new(exe_path)
        .arg("daemon")
        .spawn()
        .map_err(|e| Error::SpawnFailed(format!("failed to spawn daemon: {}", e)))?;

    Ok(())
}

fn wait_for_daemon(config: &Config) -> Result<(), Error> {
    let start = Instant::now();

    while start.elapsed() < SPAWN_TIMEOUT {
        if config.socket_path.exists()
            && std::os::unix::net::UnixStream::connect(&config.socket_path).is_ok()
        {
            return Ok(());
        }
        std::thread::sleep(POLL_INTERVAL);
    }

    Err(Error::DaemonStartTimeout)
}
