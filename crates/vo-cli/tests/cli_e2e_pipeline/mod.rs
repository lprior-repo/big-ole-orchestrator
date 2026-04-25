#![allow(clippy::redundant_pattern_matching)]

pub mod helpers;
pub mod e2e_pipeline;
pub mod parsing;
pub mod errors;
pub mod output;
pub mod config;
pub mod history;
pub mod utils;
pub mod dispatch;
pub mod registry;
pub mod exit_codes;
pub mod doctor;
pub mod misc;

pub use helpers::{create_elf_binary, create_workflow_binary, setup_project};
