mod config;
mod ssh;
mod ui;

use std::env;
use std::process::ExitCode;

use config::Connection;
use config::{ensure_config_exists, load_config, resolve_config_path};
use ssh::run_ssh;
use ui::{find_connection, print_connections, select_connection_tui};

const USAGE: &str = "Usage:
  ssh-quick-connect            # interactive mode
  ssh-quick-connect --list     # list configured machines
  ssh-quick-connect --connect <name_or_id_or_fragment>";

enum CliCommand {
    Interactive,
    List,
    Connect(String),
    Help,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("Error: {message}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let command = parse_cli_args(env::args().skip(1).collect())?;
    let config_path = resolve_config_path();

    if ensure_config_exists(&config_path).map_err(|e| e.to_string())? {
        println!(
            "Created example config at: {}\nEdit the file and run again.",
            config_path.display()
        );
        return Ok(());
    }

    let config = load_config(&config_path).map_err(|e| e.to_string())?;

    match command {
        CliCommand::Help => {
            println!("{USAGE}");
            return Ok(());
        }
        CliCommand::List => {
            print_connections(&config.connections);
            return Ok(());
        }
        CliCommand::Connect(query) => {
            let selected = select_connection(&query, &config.connections)?;
            return connect(selected);
        }
        CliCommand::Interactive => {}
    }

    let choice = select_connection_tui(&config.connections)?;
    let selected = &config.connections[choice];
    connect(selected)
}

fn select_connection<'a>(
    query: &str,
    connections: &'a [Connection],
) -> Result<&'a Connection, String> {
    let idx = find_connection(query, connections)?;
    Ok(&connections[idx])
}

fn connect(connection: &Connection) -> Result<(), String> {
    println!(
        "Connecting to '{}' ({})...",
        connection.name, connection.host
    );

    let status = run_ssh(connection).map_err(|err| {
        format!("failed to start ssh process (is 'ssh' available in PATH?): {err}")
    })?;
    if !status.success() {
        return Err(format!("ssh exited with status: {status}"));
    }
    Ok(())
}

fn parse_cli_args(args: Vec<String>) -> Result<CliCommand, String> {
    match args.as_slice() {
        [] => Ok(CliCommand::Interactive),
        [flag] if flag == "--list" || flag == "-l" => Ok(CliCommand::List),
        [flag, name] if flag == "--connect" || flag == "-c" => {
            if name.trim().is_empty() {
                Err(format!("missing connection name\n\n{USAGE}"))
            } else {
                Ok(CliCommand::Connect(name.clone()))
            }
        }
        [flag] if flag == "--help" || flag == "-h" => Ok(CliCommand::Help),
        _ => Err(format!("invalid arguments\n\n{USAGE}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{CliCommand, parse_cli_args};

    #[test]
    fn parses_list_command() {
        let command = parse_cli_args(vec!["--list".to_string()]).expect("expected --list to parse");
        assert!(matches!(command, CliCommand::List));
    }

    #[test]
    fn parses_connect_command() {
        let command = parse_cli_args(vec!["--connect".to_string(), "prod".to_string()])
            .expect("expected --connect to parse");
        match command {
            CliCommand::Connect(value) => assert_eq!(value, "prod"),
            _ => panic!("expected connect command"),
        }
    }

    #[test]
    fn rejects_unknown_arguments() {
        assert!(parse_cli_args(vec!["--unknown".to_string()]).is_err());
    }

    #[test]
    fn parses_help_command() {
        let command = parse_cli_args(vec!["--help".to_string()]).expect("expected --help to parse");
        assert!(matches!(command, CliCommand::Help));
    }
}
