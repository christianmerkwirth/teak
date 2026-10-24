use std::process::ExitCode;

use teak::cli::{self, Args};

fn main() -> ExitCode {
    let code = match cli::parse_args(std::env::args_os()) {
        Ok(Args::Version) => {
            println!("{}", teak::version_line());
            0
        }
        Ok(Args::Run(args)) => match teak::config::resolve(&args) {
            Ok((opts, warnings)) => {
                for w in warnings {
                    eprintln!("teak: {w}");
                }
                let env = teak::Env::from_process();
                let stdout = std::io::stdout();
                let mut out = std::io::BufWriter::new(stdout.lock());
                teak::run(&opts, &env, &mut out, &mut std::io::stderr())
            }
            Err(e) => {
                eprintln!("teak: {e}");
                2
            }
        },
        Err(e) => {
            let _ = e.print();
            e.exit_code() as u8
        }
    };
    #[cfg(feature = "test-hooks")]
    teak::io::hooks::write_report();
    ExitCode::from(code)
}
