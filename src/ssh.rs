use std::ffi::OsString;
use std::io;
use std::process::{Command, ExitStatus};

use crate::config::Connection;

pub fn destination(connection: &Connection) -> String {
    match &connection.user {
        Some(user) if !user.trim().is_empty() => format!("{user}@{}", connection.host),
        _ => connection.host.clone(),
    }
}

pub fn build_ssh_args(connection: &Connection) -> Vec<OsString> {
    let mut args = Vec::new();
    args.push(OsString::from(destination(connection)));

    if let Some(port) = connection.port {
        args.push(OsString::from("-p"));
        args.push(OsString::from(port.to_string()));
    }

    if let Some(identity_file) = &connection.identity_file {
        args.push(OsString::from("-i"));
        args.push(identity_file.as_os_str().to_os_string());
    }

    for option in &connection.options {
        args.push(OsString::from("-o"));
        args.push(OsString::from(option));
    }

    args
}

pub fn run_ssh(connection: &Connection) -> io::Result<ExitStatus> {
    Command::new("ssh")
        .args(build_ssh_args(connection))
        .status()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::config::Connection;
    use crate::ssh::{build_ssh_args, destination};

    #[test]
    fn destination_with_user() {
        let connection = Connection {
            id: Some("prod".to_string()),
            name: "prod".to_string(),
            host: "example.com".to_string(),
            user: Some("alice".to_string()),
            port: None,
            identity_file: None,
            options: vec![],
        };

        assert_eq!(destination(&connection), "alice@example.com");
    }

    #[test]
    fn destination_without_user() {
        let connection = Connection {
            id: Some("prod".to_string()),
            name: "prod".to_string(),
            host: "example.com".to_string(),
            user: None,
            port: None,
            identity_file: None,
            options: vec![],
        };

        assert_eq!(destination(&connection), "example.com");
    }

    #[test]
    fn builds_full_ssh_args() {
        let connection = Connection {
            id: Some("prod".to_string()),
            name: "prod".to_string(),
            host: "example.com".to_string(),
            user: Some("alice".to_string()),
            port: Some(2222),
            identity_file: Some(PathBuf::from("~/.ssh/id_ed25519")),
            options: vec![
                "StrictHostKeyChecking=accept-new".to_string(),
                "ServerAliveInterval=30".to_string(),
            ],
        };

        let args: Vec<String> = build_ssh_args(&connection)
            .iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();

        assert_eq!(
            args,
            vec![
                "alice@example.com",
                "-p",
                "2222",
                "-i",
                "~/.ssh/id_ed25519",
                "-o",
                "StrictHostKeyChecking=accept-new",
                "-o",
                "ServerAliveInterval=30",
            ]
        );
    }
}
