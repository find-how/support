use config::Repository;
use serde_json::json;
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    // Create initial config
    let mut initial = HashMap::new();
    initial.insert("app.name".to_string(), json!("MyApp"));
    initial.insert("app.debug".to_string(), json!(true));
    initial.insert("database.port".to_string(), json!(5432));

    let config = Repository::new(initial);

    // Get typed values
    let app_name = config.string("app.name").await.unwrap();
    let debug_mode = config.boolean("app.debug").await.unwrap();
    let db_port = config.integer("database.port").await.unwrap();

    println!("App name: {}", app_name);
    println!("Debug mode: {}", debug_mode);
    println!("DB port: {}", db_port);
}
