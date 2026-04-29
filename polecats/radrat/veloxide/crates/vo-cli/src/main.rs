use std::process::ExitCode;
use vo_cli::{dispatch, interpret_cli_from, map_error_to_exit_code, CliError};

fn code_to_u8(code: i32) -> u8 {
    u8::try_from(code).unwrap_or(255)
}

fn main() -> ExitCode {
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(_) => return ExitCode::from(1),
    };

    let exit_code: u8 = rt.block_on(async {
        match interpret_cli_from(std::env::args_os()) {
            Ok(cli) => match dispatch(cli).await {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("error: {e}");
                    code_to_u8(map_error_to_exit_code(&e))
                }
            },
            Err(e) => {
                let _val = e.print();
                code_to_u8(map_error_to_exit_code(&CliError::Clap(e)))
            }
        }
    });

    ExitCode::from(exit_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_code_to_u8_does_not_return_zero_for_non_zero_input() {
        assert_eq!(code_to_u8(1), 1);
        assert_eq!(code_to_u8(255), 255);
        assert_eq!(code_to_u8(256), 255); // falls back to 255
        assert_eq!(code_to_u8(-1), 255); // falls back to 255
    }
}
