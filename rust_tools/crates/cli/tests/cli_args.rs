use clap::Parser;

use cli::{AuthzBackendChoice, Cli, Command};

#[test]
fn defaults_to_cedarling_backend() {
    let cli = Cli::try_parse_from([
        "clawdia",
        "--api-key", "k",
        "chat",
    ])
    .expect("parse");
    assert!(matches!(cli.authz_backend, AuthzBackendChoice::Cedarling));
    assert!(matches!(cli.command, Command::Chat));
}

#[test]
fn yaml_backend_is_opt_in() {
    let cli = Cli::try_parse_from([
        "clawdia",
        "--api-key", "k",
        "--authz-backend", "yaml",
        "chat",
    ])
    .expect("parse");
    assert!(matches!(cli.authz_backend, AuthzBackendChoice::Yaml));
}
