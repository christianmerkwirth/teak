use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let code = if args.len() == 1 && args[0] == "--version" {
        println!("{}", teak::version_line());
        ExitCode::SUCCESS
    } else {
        // Task-2 replaces this with the clap CLI and teak::run.
        eprintln!("teak: not implemented yet");
        ExitCode::from(2)
    };
    #[cfg(feature = "test-hooks")]
    teak::io::hooks::write_report();
    code
}
