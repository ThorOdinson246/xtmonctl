use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn help_prints_usage() {
    Command::cargo_bin("xtmonctl")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("External monitor brightness control"));
}

#[test]
fn invalid_subcommand_fails() {
    Command::cargo_bin("xtmonctl")
        .unwrap()
        .arg("nope")
        .assert()
        .failure();
}

#[test]
fn set_command_is_parsed() {
    use clap::Parser;
    use xtmonctl::{Cli, Commands};

    let cli = Cli::parse_from(["xtmonctl", "set", "1", "+10"]);
    match cli.command {
        Some(Commands::Set { monitor, value }) => {
            assert_eq!(monitor, "1");
            assert_eq!(value, "+10");
        }
        _ => panic!("expected set command"),
    }
}

#[test]
fn global_options_and_alias_command_are_parsed() {
    use clap::Parser;
    use xtmonctl::cli::{AliasSubcommand, Commands};
    use xtmonctl::Cli;

    let cli = Cli::parse_from([
        "xtmonctl",
        "--json",
        "--config",
        "/tmp/xtmonctl.yaml",
        "alias",
        "set",
        "1",
        "Main Monitor",
    ]);

    assert!(cli.json);
    assert_eq!(
        cli.config.as_deref().and_then(|path| path.to_str()),
        Some("/tmp/xtmonctl.yaml")
    );
    match cli.command {
        Some(Commands::Alias(alias)) => match alias.command {
            AliasSubcommand::Set { monitor, alias } => {
                assert_eq!(monitor, "1");
                assert_eq!(alias, "Main Monitor");
            }
            _ => panic!("expected alias set command"),
        },
        _ => panic!("expected alias command"),
    }
}
