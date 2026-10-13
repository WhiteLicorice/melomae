//! The Melomae headless command line.
//!
//! The CLI is the end-to-end restore path that agents and tests use (Stack §B). It
//! chains decode, chunk sizing, inference, and encode (task 07 scope 1) and holds no
//! pipeline logic of its own. `src-tauri` and the jobs pipeline reuse the core directly.

mod restore;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// The Melomae headless command line.
#[derive(Debug, Parser)]
#[command(
    name = "melomae-cli",
    version,
    about = "Restore audio with Apollo, headless."
)]
struct Cli {
    /// The subcommand to run.
    #[command(subcommand)]
    command: Command,
}

/// The subcommands the CLI offers.
#[derive(Debug, Subcommand)]
enum Command {
    /// Restore one audio file.
    Restore(restore::RestoreArgs),
}

fn main() -> ExitCode {
    // A usage error exits here, with clap's exit code 2.
    let cli = Cli::parse();
    match cli.command {
        Command::Restore(args) => restore::run(args),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;
    use std::path::Path;

    use crate::restore::{Device, Format};

    /// Take the restore arguments out of a parsed command line.
    fn restore_args(arguments: &[&str]) -> restore::RestoreArgs {
        let cli = Cli::try_parse_from(arguments).expect("the arguments must parse");
        match cli.command {
            Command::Restore(args) => args,
        }
    }

    #[test]
    fn the_restore_command_defaults_to_flac_auto_and_no_paths() {
        let args = restore_args(&["melomae-cli", "restore", "in.flac", "out.flac"]);
        assert_eq!(args.format, Format::Flac);
        assert_eq!(args.device, Device::Auto);
        assert_eq!(args.model, None);
        assert_eq!(args.onnxruntime, None);
    }

    #[test]
    fn the_restore_command_reads_every_option() {
        let args = restore_args(&[
            "melomae-cli",
            "restore",
            "in.flac",
            "out.opus",
            "--format",
            "opus",
            "--device",
            "cpu",
            "--model",
            "apollo.onnx",
            "--onnxruntime",
            "onnxruntime.dll",
        ]);
        assert_eq!(args.input, Path::new("in.flac"));
        assert_eq!(args.output, Path::new("out.opus"));
        assert_eq!(args.format, Format::Opus);
        assert_eq!(args.device, Device::Cpu);
        assert_eq!(args.model.as_deref(), Some(Path::new("apollo.onnx")));
        assert_eq!(
            args.onnxruntime.as_deref(),
            Some(Path::new("onnxruntime.dll"))
        );
    }

    #[test]
    fn gpu_is_not_a_device_value() {
        // Task 08 adds the GPU path. Until then `gpu` is a clap usage error.
        let error = Cli::try_parse_from([
            "melomae-cli",
            "restore",
            "in.flac",
            "out.flac",
            "--device",
            "gpu",
        ])
        .expect_err("gpu must be rejected");
        assert_eq!(error.kind(), ErrorKind::InvalidValue);
    }

    #[test]
    fn an_unknown_format_is_a_usage_error() {
        let error = Cli::try_parse_from([
            "melomae-cli",
            "restore",
            "in.flac",
            "out.mp3",
            "--format",
            "mp3",
        ])
        .expect_err("mp3 output must be rejected");
        assert_eq!(error.kind(), ErrorKind::InvalidValue);
    }
}
