use std::env;
use vo_ipc::{run_subprocess, IpcError, SubprocessConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: manual_qa <driver_path> <mode> [extra args...]");
        std::process::exit(1);
    }

    let driver_path = &args[1];
    let mode = &args[2];

    match mode.as_str() {
        "smoke" => {
            println!("--- SMOKE TEST: echo-fd3 ---");
            let payload = b"echo-fd3 hello-from-parent".to_vec();

            let config = SubprocessConfig::new(driver_path, 1000, payload.clone())?;

            let result = run_subprocess(config).await;
            match result {
                Ok(output) => {
                    println!("Success!");
                    println!(
                        "FD4 Bytes: {:?}",
                        String::from_utf8_lossy(&output.fd4_bytes)
                    );
                    println!(
                        "Stderr: {:?}",
                        String::from_utf8_lossy(&output.stderr_bytes)
                    );
                    // The driver echoes the WHOLE payload back if it's "echo-fd3 ..."
                    if output.fd4_bytes == payload {
                        println!("VERDICT: PASS");
                    } else {
                        println!("VERDICT: FAIL (Payload mismatch)");
                        println!("Expected: {:?}", String::from_utf8_lossy(&payload));
                        println!("Actual:   {:?}", String::from_utf8_lossy(&output.fd4_bytes));
                    }
                }
                Err(e) => {
                    println!("Error: {:?}", e);
                    println!("VERDICT: FAIL");
                }
            }
        }
        "timeout" => {
            println!("--- INTEGRATION TEST: timeout ---");
            let timeout_ms = 500;
            // timeout-ignore means it will just sleep and ignore sigterm
            let payload = b"timeout-ignore none sleep".to_vec();

            let config = SubprocessConfig::new(driver_path, timeout_ms, payload)?;

            let result = run_subprocess(config).await;
            match result {
                Ok(_) => {
                    println!("Error: Subprocess succeeded but should have timed out");
                    println!("VERDICT: FAIL");
                }
                Err(IpcError::Timeout { elapsed_ms, .. }) => {
                    println!("Timed out correctly after {}ms", elapsed_ms);
                    println!("VERDICT: PASS");
                }
                Err(e) => {
                    println!("Unexpected error: {:?}", e);
                    println!("VERDICT: FAIL");
                }
            }
        }
        _ => {
            eprintln!("Unknown mode: {}", mode);
        }
    }

    Ok(())
}
