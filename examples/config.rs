use store::Config;

fn main() {
    // Create a new configuration repository
    let mut config = Config::new();

    // Set some configuration values
    config.set("app.name", "MyApp");
    config.set("app.debug", true);
    config.set("database.host", "localhost");
    config.set("database.port", 5432);
    config.set("database.credentials.username", "admin");
    config.set("database.credentials.password", "secret");

    // Get configuration values with type safety
    let app_name: Option<String> = config.get("app.name");
    let debug_mode: Option<bool> = config.get("app.debug");
    let db_port: Option<i32> = config.get("database.port");

    println!("App Name: {:?}", app_name);
    println!("Debug Mode: {:?}", debug_mode);
    println!("Database Port: {:?}", db_port);

    // Get multiple values at once
    let db_settings = config.get_many::<String>(vec![
        "database.host".to_string(),
        "database.credentials.username".to_string(),
    ]);

    println!("Database Settings: {:?}", db_settings);

    // Check if a configuration exists
    println!("Has app.name: {}", config.has("app.name"));
    println!("Has unknown.key: {}", config.has("unknown.key"));

    // Remove a configuration value
    config.forget("app.debug");
    println!("Has app.debug after forget: {}", config.has("app.debug"));

    // Access values using index notation
    println!("App name via index: {}", config["app.name"]);

    // Modify values using index notation
    config["app.name"] = serde_json::json!("UpdatedApp");
    println!("Updated app name: {:?}", config.get::<String>("app.name"));
}
