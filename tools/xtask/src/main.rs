//! Repository orchestration entry point (`cargo xtask ...`): golden-frame
//! generation for the state ABI.

use std::process::ExitCode;

mod error;
mod fixture;
mod output;

use output::print_line;

const USAGE: &str = "\
cargo xtask <command>

Commands:
  gen-state-fixture
      Regenerate the committed state-ABI golden frames in
      crates/indicate-instrument-state/fixtures/ from the shared
      posture fixtures.
  help
      Show this message.";

fn main() -> ExitCode {
    tracing_subscriber::fmt::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(%error, "xtask failed");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), error::XtaskError> {
    let Some((command, rest)) = args.split_first() else {
        print_line(USAGE);
        return Ok(());
    };
    match command.as_str() {
        "gen-state-fixture" => {
            if let Some(extra) = rest.first() {
                return Err(error::XtaskError::Usage {
                    message: format!("gen-state-fixture takes no arguments, got {extra:?}"),
                });
            }
            fixture::run()
        }
        "help" | "--help" | "-h" => {
            print_line(USAGE);
            Ok(())
        }
        other => Err(error::XtaskError::Usage {
            message: format!("unknown command {other:?} (expected gen-state-fixture or help)"),
        }),
    }
}
