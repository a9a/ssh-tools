use crate::config::Connection;
use dialoguer::{FuzzySelect, theme::ColorfulTheme};

pub fn print_connections(connections: &[Connection]) {
    println!("Available SSH connections:");
    for (idx, connection) in connections.iter().enumerate() {
        println!(
            "  {}. {}",
            idx + 1,
            render_connection_label(connection)
        );
    }
}

pub fn select_connection_tui(connections: &[Connection]) -> Result<usize, String> {
    let items = connections
        .iter()
        .map(render_connection_label)
        .collect::<Vec<_>>();

    FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select machine (arrows + type id/name to filter)")
        .items(&items)
        .default(0)
        .interact_opt()
        .map_err(|err| format!("failed to run terminal UI: {err}"))?
        .ok_or_else(|| "selection canceled".to_string())
}

pub fn find_connection(query: &str, connections: &[Connection]) -> Result<usize, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("connection query cannot be empty".to_string());
    }

    let query_lower = query.to_lowercase();

    if let Some((idx, _)) = connections
        .iter()
        .enumerate()
        .find(|(_, c)| c.name.eq_ignore_ascii_case(query) || has_matching_id(c, query))
    {
        return Ok(idx);
    }

    let matches: Vec<usize> = connections
        .iter()
        .enumerate()
        .filter_map(|(idx, c)| {
            let id_match = c
                .id
                .as_ref()
                .is_some_and(|id| id.to_lowercase().contains(&query_lower));
            let name_match = c.name.to_lowercase().contains(&query_lower);
            let host_match = c.host.to_lowercase().contains(&query_lower);
            if id_match || name_match || host_match {
                Some(idx)
            } else {
                None
            }
        })
        .collect();

    match matches.as_slice() {
        [idx] => Ok(*idx),
        [] => Err(format!("no connection matching '{query}'")),
        many => {
            let names = many
                .iter()
                .map(|idx| render_connection_label(&connections[*idx]))
                .collect::<Vec<_>>()
                .join(", ");
            Err(format!(
                "ambiguous connection '{query}'. Matches: {names}. Use a more specific name."
            ))
        }
    }
}

fn has_matching_id(connection: &Connection, query: &str) -> bool {
    connection
        .id
        .as_ref()
        .is_some_and(|id| id.eq_ignore_ascii_case(query))
}

pub fn render_connection_label(connection: &Connection) -> String {
    match &connection.id {
        Some(id) => format!("[{id}] {} ({})", connection.name, connection.host),
        None => format!("{} ({})", connection.name, connection.host),
    }
}

#[cfg(test)]
mod tests {
    use super::{find_connection, render_connection_label};
    use crate::config::Connection;

    fn sample_connections() -> Vec<Connection> {
        vec![
            Connection {
                id: Some("dev".to_string()),
                name: "dev-api".to_string(),
                host: "api.dev.local".to_string(),
                user: None,
                port: None,
                identity_file: None,
                options: vec![],
            },
            Connection {
                id: Some("prod-app".to_string()),
                name: "prod-app".to_string(),
                host: "app.example.com".to_string(),
                user: None,
                port: None,
                identity_file: None,
                options: vec![],
            },
            Connection {
                id: Some("prod-db".to_string()),
                name: "prod-db".to_string(),
                host: "db.example.com".to_string(),
                user: None,
                port: None,
                identity_file: None,
                options: vec![],
            },
        ]
    }

    #[test]
    fn finds_by_exact_name() {
        let connections = sample_connections();
        assert_eq!(
            find_connection("prod-app", &connections).expect("expected exact match"),
            1
        );
    }

    #[test]
    fn finds_by_id() {
        let connections = sample_connections();
        assert_eq!(
            find_connection("prod-db", &connections).expect("expected id match"),
            2
        );
    }

    #[test]
    fn finds_by_unique_fragment() {
        let connections = sample_connections();
        assert_eq!(
            find_connection("dev", &connections).expect("expected unique fragment match"),
            0
        );
    }

    #[test]
    fn rejects_ambiguous_fragment() {
        let connections = sample_connections();
        assert!(find_connection("prod", &connections).is_err());
    }

    #[test]
    fn renders_connection_label_with_id() {
        let connection = Connection {
            id: Some("prod".to_string()),
            name: "prod-api".to_string(),
            host: "api.example.com".to_string(),
            user: None,
            port: None,
            identity_file: None,
            options: vec![],
        };
        assert_eq!(
            render_connection_label(&connection),
            "[prod] prod-api (api.example.com)"
        );
    }

    #[test]
    fn renders_connection_label_without_id() {
        let connection = Connection {
            id: None,
            name: "prod-api".to_string(),
            host: "api.example.com".to_string(),
            user: None,
            port: None,
            identity_file: None,
            options: vec![],
        };
        assert_eq!(render_connection_label(&connection), "prod-api (api.example.com)");
    }

    #[test]
    fn finds_by_host_fragment() {
        let connections = sample_connections();
        assert_eq!(
            find_connection("db.example", &connections).expect("expected host match"),
            2
        );
    }
}
