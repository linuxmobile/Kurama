use std::fs;
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug)]
struct TimerState {
    active: bool,
    generation: u64,
    deadline: Instant,
}

impl Default for TimerState {
    fn default() -> Self {
        Self {
            active: false,
            generation: 0,
            deadline: Instant::now(),
        }
    }
}

#[derive(Debug, Default)]
struct ClearTimer {
    state: Mutex<TimerState>,
    wake: Condvar,
    started: AtomicBool,
    #[cfg(test)]
    spawn_count: AtomicUsize,
}

#[derive(Debug, Clone)]
pub struct ClipboardManager {
    pub wl_copy_path: String,
    pub detach_worker: bool,
    gen_path: Option<PathBuf>,
    generation: Arc<AtomicU64>,
    timer: Arc<ClearTimer>,
}

impl Default for ClipboardManager {
    fn default() -> Self {
        Self::new("wl-copy")
    }
}

impl ClipboardManager {
    pub fn new(wl_copy_path: &str) -> Self {
        let path = which::which(wl_copy_path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| wl_copy_path.to_string());
        Self {
            wl_copy_path: path,
            detach_worker: true,
            gen_path: None,
            generation: Arc::new(AtomicU64::new(1)),
            timer: Arc::new(ClearTimer::default()),
        }
    }

    pub fn without_detached_worker(mut self) -> Self {
        self.detach_worker = false;
        self
    }

    pub fn with_generation_path(mut self, path: PathBuf) -> Self {
        self.gen_path = Some(path);
        self
    }

    pub fn is_available(&self) -> bool {
        which::which(&self.wl_copy_path).is_ok()
    }

    pub fn get_generation_file_path(&self) -> PathBuf {
        self.gen_path
            .clone()
            .unwrap_or_else(|| crate::daemon::get_runtime_dir().join("clipboard_gen"))
    }

    pub fn current_generation(&self) -> u64 {
        let path = self.get_generation_file_path();
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(val) = content.trim().parse::<u64>() {
                return val;
            }
        }
        self.generation.load(Ordering::SeqCst)
    }

    pub fn next_generation(&self) -> u64 {
        let gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let path = self.get_generation_file_path();
        let _ = crate::fs_util::atomic_write_file(&path, gen.to_string().as_bytes(), 0o600);
        gen
    }

    pub fn copy(&self, text: &str, sensitive: bool, timeout_seconds: i64) -> bool {
        self.copy_with_duration(
            text,
            sensitive,
            Duration::from_secs(timeout_seconds.max(0) as u64),
        )
    }

    pub fn copy_with_duration(&self, text: &str, sensitive: bool, timeout: Duration) -> bool {
        if !self.is_available() {
            crate::log_warn!(
                "kurama:clipboard",
                "Wayland clipboard utility '{}' not found in PATH. Please install 'wl-clipboard'.",
                self.wl_copy_path
            );
            return false;
        }

        let mut cmd = Command::new(&self.wl_copy_path);
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
        if sensitive {
            cmd.arg("--sensitive");
        }

        let mut child = match cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                crate::log_error!(
                    "kurama:clipboard",
                    "Failed to spawn {}: {:?}",
                    self.wl_copy_path,
                    e
                );
                return false;
            }
        };

        let wrote = match child.stdin.take() {
            Some(mut stdin) => stdin
                .write_all(text.as_bytes())
                .and_then(|_| stdin.flush()),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "wl-copy stdin unavailable",
            )),
        };
        if let Err(e) = wrote {
            let _ = child.kill();
            let _ = child.wait();
            crate::log_error!("kurama:clipboard", "Failed to write to wl-copy: {:?}", e);
            return false;
        }

        let success = match child.wait() {
            Ok(status) => status.success(),
            Err(e) => {
                crate::log_error!("kurama:clipboard", "wl-copy process error: {:?}", e);
                false
            }
        };

        if success {
            crate::log_info!(
                "kurama:clipboard",
                "Copied content to clipboard (sensitive: {}, ttl: {:?})",
                sensitive,
                timeout
            );
            if sensitive && !timeout.is_zero() {
                self.schedule_auto_clear(timeout);
            }
        }

        success
    }

    fn schedule_auto_clear(&self, timeout: Duration) {
        let expected_gen = self.next_generation();
        let timer = Arc::clone(&self.timer);
        {
            let mut state = timer.state.lock().unwrap_or_else(|e| e.into_inner());
            state.active = true;
            state.generation = expected_gen;
            state.deadline = Instant::now() + timeout;
        }
        timer.wake.notify_all();
        if !timer.started.swap(true, Ordering::AcqRel) {
            #[cfg(test)]
            timer.spawn_count.fetch_add(1, Ordering::SeqCst);
            let copy_path = self.wl_copy_path.clone();
            let gen_path = self.get_generation_file_path();
            let local_gen = Arc::clone(&self.generation);
            let spawned = std::thread::Builder::new()
                .name("clipboard-autoclear".into())
                .spawn(move || run_clear_timer(timer, copy_path, gen_path, local_gen));
            if spawned.is_err() {
                let timer = Arc::clone(&self.timer);
                timer.started.store(false, Ordering::Release);
            }
        }

        // 2. Detached worker process: ensures timeout survives CLI parent exit without /bin/sh
        if self.detach_worker && !cfg!(test) {
            if let Ok(current_exe) = std::env::current_exe() {
                let is_kurama = current_exe
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|name| name == "kurama" || name.starts_with("kurama-"));
                if is_kurama {
                    let mut cmd = Command::new(&current_exe);
                    cmd.args([
                        "copy",
                        "--clear-after",
                        &timeout.as_secs().to_string(),
                        "--expected-gen",
                        &expected_gen.to_string(),
                    ]);
                    if self.wl_copy_path != "wl-copy" {
                        cmd.arg("--wl-copy-path").arg(&self.wl_copy_path);
                    }
                    if let Some(ref custom_gen) = self.gen_path {
                        cmd.arg("--gen-path").arg(custom_gen);
                    }
                    cmd.stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null());

                    unsafe {
                        cmd.pre_exec(|| {
                            libc::setsid();
                            Ok(())
                        });
                    }

                    let _ = cmd.spawn();
                }
            }
        }
    }

    pub fn clear(&self) -> bool {
        self.next_generation();
        if !self.is_available() {
            return false;
        }

        match Command::new(&self.wl_copy_path).arg("--clear").output() {
            Ok(output) => {
                let success = output.status.success();
                if success {
                    crate::log_info!("kurama:clipboard", "Clipboard cleared.");
                }
                success
            }
            Err(e) => {
                crate::log_error!("kurama:clipboard", "Failed to clear clipboard: {:?}", e);
                false
            }
        }
    }
}

fn run_clear_timer(
    timer: Arc<ClearTimer>,
    copy_path: String,
    gen_path: PathBuf,
    local_gen: Arc<AtomicU64>,
) {
    loop {
        let mut state = timer.state.lock().unwrap_or_else(|e| e.into_inner());
        while !state.active {
            state = timer.wake.wait(state).unwrap_or_else(|e| e.into_inner());
        }
        loop {
            let remaining = state.deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let (next, result) = timer
                .wake
                .wait_timeout(state, remaining)
                .unwrap_or_else(|e| e.into_inner());
            state = next;
            if result.timed_out() || !state.active {
                break;
            }
        }
        if !state.active {
            continue;
        }
        let expected = state.generation;
        state.active = false;
        drop(state);

        let current = fs::read_to_string(&gen_path)
            .ok()
            .and_then(|content| content.trim().parse::<u64>().ok())
            .unwrap_or_else(|| local_gen.load(Ordering::SeqCst));
        if current == expected {
            let _ = Command::new(&copy_path).arg("--clear").output();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[test]
    fn test_mock_clipboard_copy_and_clear() {
        let dir = tempdir().unwrap();
        let clip_file = dir.path().join("clipboard.txt");
        let args_file = dir.path().join("args.txt");
        let mock_wl = dir.path().join("mock-wl-copy");

        let script = format!(
            r#"#!/bin/sh
CF="{}"
AF="{}"
echo "$*" > "$AF"
if [ "$1" = "--clear" ]; then
    rm -f "$CF"
    exit 0
else
    cat > "$CF"
    exit 0
fi
"#,
            clip_file.display(),
            args_file.display()
        );

        crate::write_test_executable(&mock_wl, script);

        let mgr = ClipboardManager::new(mock_wl.to_str().unwrap());

        // Copy sensitive text -> should pass --sensitive
        assert!(mgr.copy("super_secret_password", true, 0));
        assert_eq!(
            fs::read_to_string(&clip_file).unwrap(),
            "super_secret_password"
        );
        let args = fs::read_to_string(&args_file).unwrap();
        assert!(args.contains("--sensitive"));

        // Copy non-sensitive text -> should NOT pass --sensitive
        assert!(mgr.copy("public_username", false, 0));
        assert_eq!(fs::read_to_string(&clip_file).unwrap(), "public_username");
        let args_non_sensitive = fs::read_to_string(&args_file).unwrap();
        assert!(!args_non_sensitive.contains("--sensitive"));

        // Copy multi-line report
        let multiline_report = "### Title\n\n- item 1\n- item 2\n```\nlog 1\n```\n";
        assert!(mgr.copy(multiline_report, false, 0));
        assert_eq!(fs::read_to_string(&clip_file).unwrap(), multiline_report);

        // Clear clipboard
        assert!(mgr.clear());
        assert!(!clip_file.exists());
        let clear_args = fs::read_to_string(&args_file).unwrap();
        assert!(clear_args.contains("--clear"));
    }

    #[test]
    fn test_non_existent_wl_copy() {
        let mgr = ClipboardManager::new("non_existent_wl_copy_bin_12345");
        assert!(!mgr.copy("text", false, 0));
        assert!(!mgr.clear());
    }

    #[test]
    fn test_auto_clear_timeout() {
        let dir = tempdir().unwrap();
        let clip_file = dir.path().join("clipboard.txt");
        let args_file = dir.path().join("args.txt");
        let mock_wl = dir.path().join("mock-wl-copy");

        let script = format!(
            r#"#!/bin/sh
CF="{}"
AF="{}"
echo "$*" > "$AF"
if [ "$1" = "--clear" ]; then
    rm -f "$CF"
    exit 0
else
    cat > "$CF"
    exit 0
fi
"#,
            clip_file.display(),
            args_file.display()
        );

        crate::write_test_executable(&mock_wl, script);

        let mgr = ClipboardManager::new(mock_wl.to_str().unwrap())
            .with_generation_path(dir.path().join("clipboard_gen"))
            .without_detached_worker();

        assert!(mgr.copy_with_duration("transient_secret", true, Duration::from_millis(150)));
        assert_eq!(fs::read_to_string(&clip_file).unwrap(), "transient_secret");

        // Sleep to let timer expire and clear
        std::thread::sleep(Duration::from_millis(250));
        assert!(!clip_file.exists());
    }

    #[test]
    fn test_auto_clear_reset_by_subsequent_copy() {
        let dir = tempdir().unwrap();
        let clip_file = dir.path().join("clipboard.txt");
        let args_file = dir.path().join("args.txt");
        let mock_wl = dir.path().join("mock-wl-copy");

        let script = format!(
            r#"#!/bin/sh
CF="{}"
AF="{}"
echo "$*" > "$AF"
if [ "$1" = "--clear" ]; then
    rm -f "$CF"
    exit 0
else
    cat > "$CF"
    exit 0
fi
"#,
            clip_file.display(),
            args_file.display()
        );

        crate::write_test_executable(&mock_wl, script);

        let mgr = ClipboardManager::new(mock_wl.to_str().unwrap())
            .with_generation_path(dir.path().join("clipboard_gen"))
            .without_detached_worker();

        // The first copy's deadline must sit far enough in the future that the
        // second copy finishes AND bumps the generation counter before the
        // first timer wakes: with fixed 150ms/60ms margins, a loaded CI runner
        // could stall past 150ms mid-copy and let timer #1 clear the freshly
        // written secret_2 (observed as a flake on GitHub runners). Polling on
        // the generation counter removes the race; the assertions stay the
        // same: a subsequent copy invalidates the pending auto-clear.
        let first_deadline = Duration::from_millis(2000);
        let start = std::time::Instant::now();
        assert!(mgr.copy_with_duration("secret_1", true, first_deadline));

        std::thread::sleep(Duration::from_millis(50));
        let gen_before = mgr.current_generation();
        // secret_2's own auto-clear must outlive the assertion window below.
        assert!(mgr.copy_with_duration("secret_2", true, Duration::from_millis(3000)));
        let bump_guard = start + first_deadline - Duration::from_millis(500);
        while mgr.current_generation() <= gen_before {
            assert!(
                std::time::Instant::now() < bump_guard,
                "second copy did not bump the generation counter in time"
            );
            std::thread::sleep(Duration::from_millis(10));
        }

        // Wait past secret_1's deadline
        while std::time::Instant::now() < start + first_deadline + Duration::from_millis(250) {
            std::thread::sleep(Duration::from_millis(20));
        }
        // secret_2 should STILL be present because secret_1's timer generation mismatched!
        assert_eq!(fs::read_to_string(&clip_file).unwrap(), "secret_2");

        // Poll for secret_2's auto-clear to execute and remove clip_file
        let mut cleared = false;
        for _ in 0..300 {
            if !clip_file.exists() {
                cleared = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(cleared, "clip_file was not cleared within timeout");
    }

    #[test]
    fn test_clear_cancels_pending_auto_clear() {
        let dir = tempdir().unwrap();
        let clip_file = dir.path().join("clipboard.txt");
        let args_file = dir.path().join("args.txt");
        let mock_wl = dir.path().join("mock-wl-copy");

        let script = format!(
            r#"#!/bin/sh
CF="{}"
AF="{}"
echo "$*" > "$AF"
if [ "$1" = "--clear" ]; then
    rm -f "$CF"
    exit 0
else
    cat > "$CF"
    exit 0
fi
"#,
            clip_file.display(),
            args_file.display()
        );

        crate::write_test_executable(&mock_wl, script);

        let mgr = ClipboardManager::new(mock_wl.to_str().unwrap())
            .with_generation_path(dir.path().join("clipboard_gen"))
            .without_detached_worker();

        assert!(mgr.copy_with_duration("secret_to_cancel", true, Duration::from_millis(200)));
        std::thread::sleep(Duration::from_millis(50));
        assert!(mgr.clear());
        assert!(!clip_file.exists());

        // Write arbitrary content manually to simulate user copying something unrelated
        fs::write(&clip_file, "unrelated_user_text").unwrap();

        // Wait past original auto-clear timer (200ms)
        std::thread::sleep(Duration::from_millis(250));

        // The unrelated content must NOT have been cleared because clear() invalidated the timer generation
        assert!(clip_file.exists());
        assert_eq!(
            fs::read_to_string(&clip_file).unwrap(),
            "unrelated_user_text"
        );
    }

    #[test]
    fn test_generation_isolation_and_security() {
        let dir = tempdir().unwrap();
        let gen_file = dir.path().join("clipboard_gen");
        let mgr = ClipboardManager::new("wl-copy").with_generation_path(gen_file.clone());

        let g1 = mgr.current_generation();
        let g2 = mgr.next_generation();
        assert!(g2 > g1);
        assert_eq!(mgr.current_generation(), g2);

        // Verify file permissions 0600
        let meta = fs::metadata(&gen_file).unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);
    }

    #[test]
    fn test_copy_propagates_stdin_write_failure() {
        let dir = tempdir().unwrap();
        let script = dir.path().join("wl-copy-fail");
        crate::write_test_executable(&script, "#!/bin/sh\nexit 0\n");
        let mgr = ClipboardManager::new(script.to_str().unwrap()).without_detached_worker();

        assert!(
            !mgr.copy(&"x".repeat(1024 * 1024), true, 10),
            "a failed stdin write must not be reported as a copy"
        );
    }

    #[test]
    fn test_repeated_sensitive_copies_use_one_timer() {
        let dir = tempdir().unwrap();
        let script = dir.path().join("wl-copy-mock");
        let log = dir.path().join("args.log");
        crate::write_test_executable(
            &script,
            &format!("#!/bin/sh\nprintf '%s\n' \"$*\" >> \"{}\"\nexit 0\n", log.display()),
        );
        let gen = dir.path().join("clipboard_gen");
        let mgr = ClipboardManager::new(script.to_str().unwrap())
            .without_detached_worker()
            .with_generation_path(gen.clone());

        for _ in 0..5 {
            assert!(mgr.copy("secret", true, 1));
        }
        assert_eq!(mgr.timer.spawn_count.load(Ordering::SeqCst), 1);
        assert_eq!(mgr.current_generation(), 6);
        assert!(fs::read_to_string(&log).unwrap().lines().all(|l| l == "--sensitive"));
    }
}
