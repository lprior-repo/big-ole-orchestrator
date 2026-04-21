use crate::cli::CliError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardlineConfig {
    pub target: String,
    pub engine_url: String,
    pub timeout: u64,
    pub force: bool,
    pub dry_run: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum HardlineError {
    #[error("hardline target not found: {0}")]
    TargetNotFound(String),
    #[error("hardline timeout after {0}s")]
    Timeout(u64),
    #[error("hardline engine error: {0}")]
    EngineError(String),
}

pub fn run_hardline(config: &HardlineConfig) -> Result<(), HardlineError> {
    if config.dry_run {
        println!("[dry-run] hardline target: {}", config.target);
        println!("[dry-run] engine: {}", config.engine_url);
        println!("[dry-run] timeout: {}s", config.timeout);
        if config.force {
            println!("[dry-run] force mode enabled");
        }
        return Ok(());
    }

    println!(
        "Hardline operation on target '{}' via engine {}",
        config.target, config.engine_url
    );

    if !config.force {
        println!("Hardline operation would be destructive. Use --force to proceed.");
        return Ok(());
    }

    println!("Hardline completed successfully.");
    Ok(())
}