use std::path::Path;

use super::{CategoryReport, CheckCategory, Severity};

fn is_port_available(host: &str, port: u16) -> bool {
    std::net::TcpListener::bind(format!("{host}:{port}")).is_ok()
}

pub fn check_port_availability(project_dir: &Path, _vo_dir: &Path) -> CategoryReport {
    let mut report = CategoryReport::new(CheckCategory::PortAvailability);

    let config_path = project_dir.join("config.toml");
    let default_port = 8080u16;
    let default_host = "127.0.0.1";

    let (port, host) = if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(table) = content.parse::<toml::Table>() {
                let port = table
                    .get("server")
                    .and_then(|s| s.get("port"))
                    .and_then(|p| p.as_integer())
                    .map(|p| p as u16)
                    .unwrap_or(default_port);
                let host = table
                    .get("server")
                    .and_then(|s| s.get("host"))
                    .and_then(|h| h.as_str())
                    .unwrap_or(default_host);
                (port, host.to_string())
            } else {
                (default_port, default_host.to_string())
            }
        } else {
            (default_port, default_host.to_string())
        }
    } else {
        (default_port, default_host.to_string())
    };

    if is_port_available(&host, port) {
        report.push(
            "serve-port",
            Severity::Info,
            format!("port {port} on {host} is available for serve mode"),
        );
    } else {
        report.push(
            "serve-port",
            Severity::Error,
            format!("port {port} on {host} is NOT available (already in use)"),
        );
    }

    let alt_ports = [8081, 3000, 3001];
    for alt_port in alt_ports {
        if alt_port == port {
            continue;
        }
        if is_port_available(&host, alt_port) {
            report.push(
                "alternate-port",
                Severity::Info,
                format!("alternate port {alt_port} on {host} is available"),
            );
        }
    }

    report
}
