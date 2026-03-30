use std::process::ExitCode;
use vo_cli::{dispatch, interpret_cli_from, map_error_to_exit_code};

fn main() -> ExitCode {
    match interpret_cli_from(std::env::args_os()) {
        Ok(cli) => {
            if let Err(e) = dispatch(cli) {
                let code = map_error_to_exit_code(&e);
                match u8::try_from(code) {
                    Ok(c) => ExitCode::from(c),
                    Err(_) => ExitCode::from(255),
                }
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(e) => {
            let _ = e.print();
            let code = map_error_to_exit_code(&vo_cli::CliError::Clap(e));
            match u8::try_from(code) {
                Ok(c) => ExitCode::from(c),
                Err(_) => ExitCode::from(255),
            }
        }
    }
}
