//! zcheck command-line entrypoint.

mod app;

fn main() {
    #[cfg(windows)]
    if let Some(exit_code) = app::run_if_requested() {
        std::process::exit(exit_code);
    }
    std::process::exit(app::run());
}
