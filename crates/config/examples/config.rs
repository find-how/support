use config::Repository;

#[tokio::main]
async fn main() {
    // Create a new configuration repository
    let config = Repository::new(Default::default());

    // Set some configuration values
    config.set("app.name", "MyApp").await.unwrap();
    config.set("app.debug", true).await.unwrap();
    config.set("database.host", "localhost").await.unwrap();
    config.set("database.port", 5432).await.unwrap();
    config.set("database.credentials.username", "admin").await.unwrap();
    config.set("database.credentials.password", "secret").await.unwrap();

    // Get configuration values with type safety
    let app_name: Option<String> = config.get("app.name").await;
    let debug_mode: Option<bool> = config.get("app.debug").await;
    let db_port: Option<i32> = config.get("database.port").await;

    println!("App Name: {:?}", app_name);
    println!("Debug Mode: {:?}", debug_mode);
    println!("Database Port: {:?}", db_port);

    // Get multiple values at once
    let db_settings = config.get_many::<String>(vec![
        "database.host".to_string(),
        "database.credentials.username".to_string(),
    ]).await;

    println!("Database Settings: {:?}", db_settings);

    // Check if a configuration exists
    println!("Has app.name: {}", config.has("app.name").await);
    println!("Has unknown.key: {}", config.has("unknown.key").await);

    // Get all configuration values
    let all = config.all().await;
    println!("All settings: {:?}", all);
}
