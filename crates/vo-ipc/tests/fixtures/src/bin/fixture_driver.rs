#![allow(clippy::all, dead_code)]

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::fd::FromRawFd;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

static RECEIVED_SIGTERM: AtomicBool = AtomicBool::new(false);
static RECEIVED_SIGPIPE: AtomicBool = AtomicBool::new(false);

extern "C" fn handle_sigterm(_signal: i32) {
    RECEIVED_SIGTERM.store(true, Ordering::SeqCst);
}

extern "C" fn handle_sigpipe(_signal: i32) {
    RECEIVED_SIGPIPE.store(true, Ordering::SeqCst);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map_or("", String::as_str);

    match command {
        "echo-fd3" => command_echo_fd3(),
        "fd3-eof" => command_fd3_eof(),
        "stderr-text" => command_stderr_text(&args),
        "stderr-repeat" => command_stderr_repeat(&args),
        "read-env" => command_read_env(),
        "read-argv" => command_read_argv(&args),
        "sleep-exit" => command_sleep_exit(&args),
        "stderr-sleep-exit" => command_stderr_sleep_exit(&args),
        "timeout-term-exit" => command_timeout_term_exit(&args),
        "timeout-ignore" => command_timeout_ignore(&args),
        "pid-and-exit" => command_pid_and_exit(&args),
        "grandchild-hold" => command_grandchild_hold(&args),
        "hold-open" => command_hold_open(&args),
        "close-fd3" => command_close_fd3(),
        _ => {}
    }
}

fn command_echo_fd3() {
    let payload = read_fd3_all();
    write_fd4_envelope(&payload);
}

fn command_fd3_eof() {
    let payload = read_fd3_all();
    let mut bytes = payload;
    bytes.extend_from_slice(b"|EOF");
    let prefix_len = "fd3-eof ".len();
    let response = bytes.get(prefix_len..).unwrap_or(&bytes).to_vec();
    write_fd4_envelope(&response);
}

fn command_stderr_text(args: &[String]) {
    let text = args.get(2).map_or("", String::as_str);
    let exit_code = args
        .get(3)
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(0);
    drop(std::io::stderr().write_all(text.as_bytes()));
    std::process::exit(exit_code);
}

fn command_stderr_sleep_exit(args: &[String]) {
    let text = args.get(2).map_or("", String::as_str);
    let delay_ms = args
        .get(3)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let exit_code = args
        .get(4)
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(0);
    let _ = std::io::stderr().write_all(text.as_bytes());
    std::io::stderr().flush().unwrap();
    std::thread::sleep(Duration::from_millis(delay_ms));
    std::process::exit(exit_code);
}

fn command_stderr_repeat(args: &[String]) {
    let count = args
        .get(2)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let byte = args
        .get(3)
        .and_then(|value| value.as_bytes().first().copied())
        .unwrap_or(b'x');
    let exit_code = args
        .get(4)
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(0);
    let stderr = vec![byte; count];
    drop(std::io::stderr().write_all(&stderr));
    std::process::exit(exit_code);
}

fn command_read_env() {
    // Explicitly remove test isolation variables that may leak despite env_clear()
    // This addresses flakiness under coverage tools (llvm-cov) which may inject env vars
    env::remove_var("LEAK_ME");
    let environment: BTreeMap<String, String> = env::vars().collect();
    let payload = serde_json::to_vec(&environment).unwrap();
    write_fd4_envelope(&payload);
}

fn command_read_argv(args: &[String]) {
    let argv = args[1..].to_vec();
    let payload = serde_json::to_vec(&argv).unwrap();
    write_fd4_envelope(&payload);
}

fn command_sleep_exit(args: &[String]) {
    let delay_ms = args
        .get(2)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let exit_code = args
        .get(3)
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(0);
    let payload = args
        .get(4)
        .map_or_else(Vec::new, |value| value.as_bytes().to_vec());
    std::thread::sleep(Duration::from_millis(delay_ms));
    if !payload.is_empty() {
        write_fd4_envelope(&payload);
    }
    std::process::exit(exit_code);
}

fn command_timeout_term_exit(args: &[String]) {
    install_handler(libc::SIGTERM, handle_sigterm);
    let marker_path = args.get(2).cloned().unwrap_or_default();
    let delay_after_term = args
        .get(3)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let pid_path = args.get(4).cloned().unwrap_or_default();
    let stderr_prefix = args.get(5).cloned().unwrap_or_default();

    if pid_path != "none" {
        drop(fs::write(&pid_path, std::process::id().to_string()));
    }

    if !stderr_prefix.is_empty() {
        drop(std::io::stderr().write_all(stderr_prefix.as_bytes()));
    }

    loop {
        if RECEIVED_SIGTERM.load(Ordering::SeqCst) {
            drop(fs::write(&marker_path, "SIGTERM"));
            drop(std::io::stderr().write_all(b"sigterm"));
            std::thread::sleep(Duration::from_millis(delay_after_term));
            std::process::exit(91);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn command_timeout_ignore(args: &[String]) {
    install_handler(libc::SIGTERM, handle_sigterm);
    install_handler(libc::SIGPIPE, handle_sigpipe);
    let pid_path = args.get(2).cloned().unwrap_or_default();
    let mode = args.get(3).map_or("sleep", String::as_str);

    if pid_path != "none" {
        drop(fs::write(&pid_path, std::process::id().to_string()));
    }

    match mode {
        "flood" => {
            let chunk = vec![b'x'; 16_384];
            loop {
                if RECEIVED_SIGPIPE.load(Ordering::SeqCst) {
                    std::process::exit(77);
                }
                std::io::stderr().write_all(&chunk).expect("failed to flood stderr");
            }
        }
        _ => loop {
            RECEIVED_SIGTERM.load(Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(50));
        },
    }
}

fn command_pid_and_exit(args: &[String]) {
    let pid_path = args.get(2).cloned().unwrap_or_default();
    let exit_code = args
        .get(3)
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(0);
    drop(fs::write(pid_path, std::process::id().to_string()));
    write_fd4_envelope(b"pid-ready");
    std::process::exit(exit_code);
}

fn command_grandchild_hold(args: &[String]) {
    let sleep_ms = args
        .get(2)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(1000);
    drop(set_cloexec(3));
    drop(set_cloexec(4));
    let current = env::current_exe().unwrap();
    drop(Command::new(current)
        .args(["hold-open", &sleep_ms.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn());
    write_fd4_envelope(b"child-done");
}

fn command_hold_open(args: &[String]) {
    let sleep_ms = args
        .get(2)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(1000);
    let deadline = Instant::now() + Duration::from_millis(sleep_ms);
    for _ in 0..10000 { // NASA/JPL Rule 2: Explicit iteration ceiling
        if Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn command_close_fd3() {
    drop(close_fd3());
    write_fd4_envelope(b"closed-fd3");
}

fn read_fd3_all() -> Vec<u8> {
    let mut file = fd3_file();
    let mut bytes = Vec::new();
    drop(file.read_to_end(&mut bytes));
    bytes
}

fn write_fd4_envelope(payload: &[u8]) {
    let mut file = fd4_file();
    let length = u32::try_from(payload.len())
        .unwrap_or(u32::MAX)
        .to_be_bytes();
    drop(file.write_all(&length));
    drop(file.write_all(payload));
    drop(file.flush());
}

fn install_handler(signal: i32, handler: extern "C" fn(i32)) {
    unsafe {
        libc::signal(signal, handler as usize);
    }
}

fn set_cloexec(fd: i32) -> std::io::Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags == -1 {
        return Err(std::io::Error::last_os_error());
    }

    let outcome = unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) };
    (outcome != -1)
        .then_some(())
        .ok_or_else(std::io::Error::last_os_error)
}

fn close_fd3() -> std::io::Result<()> {
    let outcome = unsafe { libc::close(3) };
    (outcome == 0)
        .then_some(())
        .ok_or_else(std::io::Error::last_os_error)
}

fn fd3_file() -> File {
    unsafe { File::from_raw_fd(3) }
}

fn fd4_file() -> File {
    unsafe { File::from_raw_fd(4) }
}

#[allow(dead_code)]
fn _write_marker(path: &Path, value: &str) {
    drop(fs::write(path, value));
}
