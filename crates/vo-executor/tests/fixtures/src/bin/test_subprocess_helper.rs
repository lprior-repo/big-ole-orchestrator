#![allow(clippy::all, dead_code)]
use std::io::{Read, Write};
use std::os::fd::FromRawFd;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map_or("", String::as_str);

    match command {
        "echo" => command_echo(),
        "sleep-exit" => command_sleep_exit(&args),
        "grandchild-hold" => command_grandchild_hold(&args),
        "memory-bomb" => command_memory_bomb(&args),
        _ => command_echo(),
    }
}

fn command_echo() {
    let payload = read_fd3_frame();
    write_fd4_envelope(&payload);
}

fn command_sleep_exit(args: &[String]) {
    let delay_ms = args.get(2).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    let exit_code = args.get(3).and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
    let payload = args.get(4).map_or(Vec::new(), |v| v.as_bytes().to_vec());
    if !payload.is_empty() {
        write_fd4_envelope(&payload);
    }
    if delay_ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
    }
    std::process::exit(exit_code);
}

fn command_grandchild_hold(args: &[String]) {
    let sleep_ms = args
        .get(2)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1000);
    drop(set_cloexec(3));
    drop(set_cloexec(4));
    let current = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from(""));
    drop(
        std::process::Command::new(&current)
            .args(["hold-open", &sleep_ms.to_string()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn(),
    );
    write_fd4_envelope(b"child-done");
}

fn command_memory_bomb(args: &[String]) {
    let size = args
        .get(2)
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    let payload = vec![b'x'; size];
    write_fd4_envelope(&payload);
    std::process::exit(0);
}

fn read_fd3_frame() -> Vec<u8> {
    let mut file = fd3_file();
    let mut len_buf = [0u8; 4];
    let _ = file.read(&mut len_buf);
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    let _ = file.read_exact(&mut payload);
    payload
}

fn write_fd4_envelope(payload: &[u8]) {
    let mut file = fd4_file();
    let length = u32::try_from(payload.len())
        .unwrap_or(u32::MAX)
        .to_be_bytes();
    let _ = file.write_all(&length);
    let _ = file.write_all(payload);
    let _ = file.flush();
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

fn fd3_file() -> std::fs::File {
    unsafe { std::fs::File::from_raw_fd(3) }
}

fn fd4_file() -> std::fs::File {
    unsafe { std::fs::File::from_raw_fd(4) }
}
