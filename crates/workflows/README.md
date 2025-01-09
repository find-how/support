# Securely Workflows

A Laravel-inspired workflow engine for the Securely server, providing a
comprehensive system for defining, executing, and managing workflows with a
powerful CLI interface.

## Features

- **Workflow Engine**:
  - Stateful workflow management
  - Step-based execution with retry strategies
  - Activity handlers for task execution
  - Async execution and worker pooling
  - Child workflow support
  - Versioning and migration capabilities

- **Storage Layer**:
  - Persistent storage using Sled
  - Event sourcing for workflow history
  - Snapshot support for state management
  - Transactional operations

- **Activity System**:
  - Pluggable activity handlers
  - Async execution support
  - Retry strategies with backoff
  - Compensation handling for rollbacks

- **CLI Interface**:
  - Create and manage workflows
  - Schedule recurring workflows
  - Monitor execution status
  - Send signals and queries
  - Manage child workflows
  - Handle versioning and migrations

- **Monitoring**:
  - Prometheus metrics integration
  - Execution statistics
  - Performance monitoring
  - Health checks

## Installation

Add to your Cargo.toml:

```toml
[dependencies]
securely-workflows = "0.1"
```

## Usage

### CLI Commands

```bash
# Create a workflow
workflows create --definition workflow.json --priority High --tags dev,test

# List workflows
workflows list --status Running --tag dev

# Show workflow details
workflows show --id <workflow-id>

# Execute specific step
workflows execute --id <workflow-id> --step-id <step-id>

# Send signals
workflows send-signal --id <workflow-id> --signal UpdateData --data "key1=value1" "key2=value2"

# Query workflow
workflows query --id <workflow-id> --query GetStatus

# Schedule workflow
workflows schedule --definition workflow.json --cron "0 * * * * *" --tags scheduled

# Manage child workflows
workflows create-child --parent-id <parent-id> --definition child.json
workflows list-children --parent-id <parent-id>

# Version migration
workflows migrate --id <workflow-id> --version 2
```

### Workflow Definition Example

```json
{
  "steps": [
    {
      "id": "step1",
      "name": "Process File",
      "handler": "FileProcessing",
      "retry_strategy": {
        "max_attempts": 3,
        "backoff_initial": 1,
        "backoff_factor": 2.0,
        "backoff_max": 30
      },
      "compensation": {
        "handler": "DeleteProcessedFile"
      }
    },
    {
      "id": "step2",
      "name": "API Call",
      "handler": "ApiCall",
      "dependencies": ["step1"]
    }
  ],
  "data": {
    "file_path": "/path/to/file",
    "api_endpoint": "https://api.example.com"
  }
}
```

## Architecture

The crate is organized into several key components:

1. **Models**: Core data structures for workflows, steps, and activities
2. **Storage**: Persistence layer with event sourcing
3. **Engine**: Workflow execution and management
4. **Activities**: Task execution handlers
5. **CLI**: Command-line interface
6. **Metrics**: Prometheus integration

## Development

### Building

```bash
cargo build --release
```

### Testing

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run benchmarks
cargo bench
```

### Documentation

```bash
cargo doc --no-deps --open
```

## Laravel Equivalents

This implementation draws inspiration from Laravel's workflow patterns while
embracing Rust's strengths:

- **Async Execution**: Uses Tokio for async operations
- **Type Safety**: Leverages Rust's type system
- **Memory Safety**: Ownership and borrowing patterns
- **Concurrency**: Safe concurrent execution with worker pools

## Contributing

1. Fork the repository
2. Create your feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

## License

MIT License
