use std::env;
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use fs2::FileExt;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn executable_name() -> &'static str {
    if cfg!(windows) {
        "aoe4_replay_launcher.exe"
    } else {
        "aoe4_replay_launcher"
    }
}

pub(crate) fn install_path() -> io::Result<PathBuf> {
    #[cfg(windows)]
    let dir = dirs::data_local_dir()
        .ok_or_else(|| io::Error::other("LocalAppData directory not found"))?
        .join("AOE4ReplayLauncher");
    #[cfg(target_os = "linux")]
    let dir = dirs::home_dir()
        .ok_or_else(|| io::Error::other("Home directory not found"))?
        .join(".local/bin");
    Ok(dir.join(executable_name()))
}

fn open_lock(dir: &Path) -> io::Result<File> {
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join(".aoe4-replay-launcher.lock"))
}

fn newer(candidate: &str, installed: &str) -> Result<bool> {
    Ok(semver::Version::parse(candidate.trim())? > semver::Version::parse(installed.trim())?)
}

// Stage beside the destination so installation is an atomic rename on the same filesystem.
fn install_copy(source: &Path, destination: &Path) -> Result<()> {
    let staged = tempfile::NamedTempFile::new_in(destination.parent().ok_or("Missing parent")?)?;
    fs::copy(source, staged.path())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(staged.path(), fs::Permissions::from_mode(0o755))?;
    }
    staged.as_file().sync_all()?;
    staged.persist(destination)?;
    Ok(())
}

/// Returns the installed child's exit code when launched from a download/build directory.
pub fn install_and_relaunch() -> Result<Option<i32>> {
    let current = env::current_exe()?;
    let installed = install_path()?;
    if installed.exists() && fs::canonicalize(&current)? == fs::canonicalize(&installed)? {
        return Ok(None);
    }
    let dir = installed.parent().ok_or("Missing installation directory")?;
    fs::create_dir_all(dir)?;
    let lock = open_lock(dir)?;
    lock.lock_exclusive()?;
    let replace = if installed.exists() {
        let output = Command::new(&installed).arg("--version").output()?;
        if !output.status.success() {
            return Err("Could not read installed launcher version".into());
        }
        newer(
            env!("CARGO_PKG_VERSION"),
            std::str::from_utf8(&output.stdout)?,
        )?
    } else {
        true
    };
    if replace {
        install_copy(&current, &installed)?;
        println!("Installed launcher at {}", installed.display());
    }
    drop(lock);
    // Register the installed path even when migrating an old handler during a replay launch.
    crate::register_url_protocol_at(&installed)?;
    let status = Command::new(&installed)
        .args(env::args_os().skip(1))
        .status()?;
    Ok(Some(status.code().unwrap_or(1)))
}

pub fn update_on_launch() -> Result<()> {
    // Published releases currently contain only x86-64 executables.
    if !cfg!(target_arch = "x86_64") {
        return Ok(());
    }
    let installed = install_path()?;
    let lock = open_lock(installed.parent().ok_or("Missing installation directory")?)?;
    match lock.try_lock_exclusive() {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
        Err(error) => return Err(error.into()),
    }
    if let Some(version) = check_and_update(update_builder())? {
        println!(
            "Launcher updated to {}. It will be used on the next launch.",
            version
        );
    }
    Ok(())
}

fn check_and_update(
    mut builder: self_update::backends::github::UpdateBuilder,
) -> Result<Option<String>> {
    // GitHub's latest endpoint excludes drafts and prereleases.
    let releases = builder.build()?.get_latest_release()?;
    let latest = releases.latest().ok_or("No release found")?;
    if !newer(latest.version(), env!("CARGO_PKG_VERSION"))? {
        return Ok(None);
    }
    let status = builder
        .release_tag(format!("v{}", latest.version()))
        .build()?
        .update()?;
    Ok(status.is_updated().then(|| status.version().to_string()))
}

fn update_builder() -> self_update::backends::github::UpdateBuilder {
    let asset_name = if cfg!(windows) {
        "aoe4-replay-launcher-windows-x86_64.zip"
    } else {
        "aoe4-replay-launcher-linux-x86_64.zip"
    };
    let mut builder = self_update::backends::github::Update::configure();
    builder
        .repo_owner("aoe4replays-gg")
        .repo_name("launcher")
        .bin_name(executable_name())
        .bin_path_in_archive(executable_name())
        .current_version(env!("CARGO_PKG_VERSION"))
        .asset_matcher(move |assets| {
            assets
                .iter()
                .find(|asset| asset.name() == asset_name)
                .cloned()
        })
        .checksum_from_asset("SHA256SUMS")
        .timeout(Duration::from_secs(15))
        .no_confirm(true)
        .show_output(false);
    builder
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::io::{BufRead, BufReader, Cursor, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    // Exercise the actual GitHub download/extract/verify pipeline against a local release.
    fn simulated_update(valid_checksum: bool) {
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join(executable_name());
        fs::write(&destination, b"old executable").unwrap();
        let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        archive
            .start_file(
                executable_name(),
                zip::write::SimpleFileOptions::default().unix_permissions(0o755),
            )
            .unwrap();
        archive.write_all(b"new executable").unwrap();
        let archive = archive.finish().unwrap().into_inner();
        let asset = if cfg!(windows) {
            "aoe4-replay-launcher-windows-x86_64.zip"
        } else {
            "aoe4-replay-launcher-linux-x86_64.zip"
        };
        let digest = if valid_checksum {
            Sha256::digest(&archive)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        } else {
            "0".repeat(64)
        };
        let sums = format!("{digest}  {asset}\n").into_bytes();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let release = serde_json::json!({
            "tag_name": "v99.0.0", "created_at": "2026-09-17T00:00:00Z",
            "assets": [
                {"name": asset, "url": format!("{base}/archive")},
                {"name": "SHA256SUMS", "url": format!("{base}/sums")}
            ]
        })
        .to_string()
        .into_bytes();
        let done = Arc::new(AtomicBool::new(false));
        let server_done = done.clone();
        let server = std::thread::spawn(move || {
            while !server_done.load(Ordering::Relaxed) {
                let (mut stream, _) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => panic!("{error}"),
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request = String::new();
                reader.read_line(&mut request).unwrap();
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                        break;
                    }
                }
                let body = match request.split_whitespace().nth(1).unwrap() {
                    "/repos/aoe4replays-gg/launcher/releases/latest"
                    | "/repos/aoe4replays-gg/launcher/releases/tags/v99.0.0" => &release,
                    "/archive" => &archive,
                    "/sums" => &sums,
                    path => panic!("Unexpected request: {path}"),
                };
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(body).unwrap();
            }
        });
        let mut builder = update_builder();
        builder
            .api_base_url(base)
            .bin_install_path(&destination)
            .timeout(Duration::from_secs(3));
        let result = check_and_update(builder);
        done.store(true, Ordering::Relaxed);
        server.join().unwrap();
        if valid_checksum {
            assert_eq!(result.unwrap().as_deref(), Some("99.0.0"));
            assert_eq!(fs::read(destination).unwrap(), b"new executable");
        } else {
            assert!(result.unwrap_err().to_string().contains("checksum"));
            assert_eq!(fs::read(destination).unwrap(), b"old executable");
        }
    }

    #[test]
    fn verified_release_replaces_installed_binary() {
        simulated_update(true);
    }

    #[test]
    fn bad_checksum_keeps_installed_binary() {
        simulated_update(false);
    }

    #[test]
    fn downloaded_copies_do_not_downgrade_an_installation() {
        assert!(!newer("0.1.2", "0.2.0\n").unwrap());
        assert!(!newer("0.1.2", "0.1.2").unwrap());
        assert!(newer("0.1.10", "0.1.9").unwrap());
        assert!(newer("0.1.2", "invalid").is_err());
    }

    #[test]
    fn installation_replaces_a_file_and_preserves_the_download() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("download");
        let destination = dir.path().join("installed");
        fs::write(&source, b"new executable").unwrap();
        fs::write(&destination, b"old executable").unwrap();
        install_copy(&source, &destination).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"new executable");
        assert_eq!(fs::read(&source).unwrap(), b"new executable");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_ne!(
                fs::metadata(destination).unwrap().permissions().mode() & 0o111,
                0
            );
        }
    }
}
