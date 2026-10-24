use std::process::ExitCode;

use teak::cli::{self, Parsed};

fn main() -> ExitCode {
    let code = match cli::parse_from(std::env::args_os()) {
        Ok(Parsed::Version) => {
            println!("{}", teak::version_line());
            0
        }
        Ok(Parsed::Run(opts)) => {
            let env = teak::Env::from_process();
            let stdout = std::io::stdout();
            let mut out = std::io::BufWriter::new(stdout.lock());
            teak::run(&opts, &env, &mut out, &mut std::io::stderr())
        }
        Err(e) => {
            let _ = e.print();
            e.exit_code() as u8
        }
    };
    #[cfg(feature = "test-hooks")]
    teak::io::hooks::write_report();
    ExitCode::from(code)
}
