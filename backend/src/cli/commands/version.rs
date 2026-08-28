use crate::cli::context::CliContext;
use crate::cli::error::CliError;
use serde::Serialize;

#[derive(Serialize)]
struct VersionOutput {
    name: &'static str,
    version: &'static str,
    target_arch: &'static str,
    target_os: &'static str,
    profile: &'static str,
    features: &'static [&'static str],
}

pub async fn handle(ctx: &CliContext) -> Result<(), CliError> {
    let version_data = VersionOutput {
        name: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        target_arch: std::env::consts::ARCH,
        target_os: std::env::consts::OS,
        profile: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        features: &[
            "sqlite",
            "opendal-fs",
            "opendal-s3",
            "opendal-ftp",
            "opendal-sftp",
            "argon2",
            "tokio-full",
            "axum-ws",
        ],
    };

    ctx.output.print_success("version", &version_data, || {
        println!("AeroFS v{}", env!("CARGO_PKG_VERSION"));
        println!("  • Name: {}", env!("CARGO_PKG_NAME"));
        println!(
            "  • Platform: {}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        );
        println!(
            "  • Build Profile: {}",
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
        );
        println!("  • Providers: Local Filesystem, AWS S3, FTP, SFTP");
        println!("  • Security: Argon2id, Session HMAC, Strict Path Sandboxing");
    });

    Ok(())
}
