use std::process::ExitCode;

fn main() -> ExitCode {
    // `cli::run` owns argument parsing, logging setup and error rendering.
    // We only translate the numeric exit code into a process exit status.
    let code = nodkray::cli::run();
    ExitCode::from(code.clamp(0, u8::MAX as i32) as u8)
}
