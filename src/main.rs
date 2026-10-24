use std::env;
mod updater;
#[cfg(target_os = "linux")]
use std::fs;
use std::fs::File;
use std::io::{self, Cursor, copy};
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;
use reqwest::blocking::Client;

#[cfg(windows)]
use winreg::RegKey;
#[cfg(windows)]
use winreg::enums::*;

const HOME_URL: &str = "https://aoe4replays.gg";
const AOE4_APP_ID: &str = "1466860";
// Lets the server tell launcher versions apart.
const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

fn main() {
    // Keep this side-effect free: installation uses it to avoid downgrading a newer copy.
    if env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--version")) {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return;
    }
    // Only the installed copy updates itself; other copies must not replace their own file.
    let installed = match updater::install_and_relaunch() {
        Ok(updater::Launch::Relaunched(code)) => std::process::exit(code),
        Ok(updater::Launch::Installed) => true,
        Ok(updater::Launch::InPlace) => false,
        Err(error) => {
            eprintln!("Failed to install launcher; running this copy instead: {error}");
            false
        }
    };
    println!("AoE4 Replay Launcher v{}", env!("CARGO_PKG_VERSION"));
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        let url = args[1].to_string();
        println!("URL = {url}");
        let result = download_replay(&url).and_then(run_replay);
        if let Err(error) = &result {
            eprintln!("Failed to start replay: {error}");
        }
        // Update even after a failure: a newer launcher may be the fix.
        let updated = installed && update();
        if result.is_err() {
            wait_for_key();
            std::process::exit(1);
        }
        // The window closes on exit; leave time to read the update message.
        if updated {
            std::thread::sleep(std::time::Duration::from_secs(3));
        }
    } else {
        println!("Configuring...");
        if let Err(e) = register_url_protocol(installed) {
            eprintln!("Failed to configure aoe4rep protocol: {}", e);
        } else {
            println!("Configuration done, you can now launch replays from {HOME_URL}");
        }
        if installed {
            update();
        }
        wait_for_key();
    }
}

// Runs after the replay starts so a slow or unreachable GitHub never delays it.
// Returns whether a new version was installed.
fn update() -> bool {
    println!("Checking for updates...");
    match updater::update_on_launch() {
        Ok(true) => true,
        Ok(false) => {
            println!("Launcher is up to date");
            false
        }
        Err(error) => {
            eprintln!("Update unavailable; continuing with this version: {error}");
            false
        }
    }
}

// Point the handler at the copy that is actually running when installation was skipped or failed.
fn register_url_protocol(installed: bool) -> io::Result<()> {
    let exe = if installed {
        updater::install_path()?
    } else {
        env::current_exe()?
    };
    register_url_protocol_at(&exe)
}

fn wait_for_key() {
    println!("press Enter to close this window...");
    let mut dummy = String::new();
    io::stdin().read_line(&mut dummy).expect("");
}

fn download_replay(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let filename = format!("AgeIV_Replay_{}", timestamp);
    let folder = playback_dir()?;
    println!("Replay playback folder detected in : {}", folder.display());
    let mut file_path = folder.clone();
    file_path.push(&filename);
    let response = Client::builder()
        .user_agent(USER_AGENT)
        .build()?
        .get(replay_download_url(url)?)
        .send()?;
    if response.status() == reqwest::StatusCode::FORBIDDEN {
        return Err("Replays must be started from aoe4replays.gg.".into());
    }
    let bytes = response.error_for_status()?.bytes()?;
    let cursor = Cursor::new(bytes);
    let mut decoder = GzDecoder::new(cursor);
    let mut output = File::create(file_path)?;
    copy(&mut decoder, &mut output)?;
    println!("Downloaded replay file {}", filename);
    Ok(filename)
}

fn replay_download_url(url: &str) -> Result<reqwest::Url, Box<dyn std::error::Error>> {
    Ok(reqwest::Url::parse_with_params(
        &format!("{HOME_URL}/api/replays"),
        &[("url", url)],
    )?)
}

// ── Windows ──────────────────────────────────────────────────────────────────

#[cfg(windows)]
fn playback_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut folder = dirs::document_dir().ok_or("Could not find Documents directory")?;
    folder.push("My Games");
    folder.push("Age of Empires IV");
    folder.push("playback");
    Ok(folder)
}

#[cfg(windows)]
fn run_replay(replay_name: String) -> Result<(), Box<dyn std::error::Error>> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let steam_key = hkcu
        .open_subkey("Software\\Valve\\Steam")
        .map_err(|e| format!("Steam entry not found in registry: {e}"))?;
    let steam_exe: String = steam_key
        .get_value("SteamExe")
        .map_err(|e| format!("SteamExe entry not found in registry: {e}"))?;
    println!("Steam located in {steam_exe}");
    println!("Starting AOE4...");
    Command::new(steam_exe)
        .args([
            "-applaunch",
            AOE4_APP_ID,
            "-dev",
            "-replay",
            &format!("playback:{}", replay_name),
        ])
        // Don't wait: when Steam isn't running yet, this process is the Steam client itself.
        .spawn()
        .map_err(|e| format!("Failed to launch Steam: {e}"))?;
    Ok(())
}

#[cfg(windows)]
fn register_url_protocol_at(exe_path: &std::path::Path) -> std::io::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey("Software\\Classes\\aoe4rep")?;
    key.set_value("", &"URL:AOE4Rep Protocol")?;
    key.set_value("URL Protocol", &"")?;
    let (command_key, _) = key.create_subkey(r"shell\open\command")?;
    command_key.set_value("", &format!("\"{}\" \"%1\"", exe_path.to_str().unwrap()))?;
    println!("Protocol 'aoe4rep' registered successfully for the current user!");
    Ok(())
}

// ── Linux ─────────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn playback_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let steam_install = find_steam_installation()?;
    let aoe4_library = find_aoe4_library(&steam_install)?;
    // AoE4 runs under Proton; replay files live inside the Proton prefix.
    let path = aoe4_library
        .join("steamapps/compatdata")
        .join(AOE4_APP_ID)
        .join("pfx/drive_c/users/steamuser/Documents/My Games/Age of Empires IV/playback");
    fs::create_dir_all(&path)?;
    Ok(path)
}

// Returns the main Steam installation directory (where libraryfolders.vdf lives).
#[cfg(target_os = "linux")]
fn find_steam_installation() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let candidates = [
        home.join(".local/share/Steam"),
        home.join(".steam/debian-installation"),
        home.join(".steam/steam"),
        home.join(".steam/root"),
    ];
    for candidate in &candidates {
        if candidate.join("steamapps/libraryfolders.vdf").exists() {
            return Ok(candidate.clone());
        }
    }
    Err("Steam installation not found. Checked: ~/.local/share/Steam, ~/.steam/debian-installation, ~/.steam/steam, ~/.steam/root".into())
}

// Parses libraryfolders.vdf and returns the library path that contains AoE4's compatdata.
#[cfg(target_os = "linux")]
fn find_aoe4_library(steam_install: &PathBuf) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let vdf = fs::read_to_string(steam_install.join("steamapps/libraryfolders.vdf"))?;
    let library_paths = parse_library_paths_from_vdf(&vdf);
    for lib in &library_paths {
        if lib.join("steamapps/compatdata").join(AOE4_APP_ID).exists() {
            return Ok(lib.clone());
        }
    }
    Err(format!(
        "AoE4 (app {AOE4_APP_ID}) not found in any Steam library. Make sure the game is installed and has been launched at least once."
    ).into())
}

#[cfg(any(target_os = "linux", test))]
fn parse_library_paths_from_vdf(content: &str) -> Vec<PathBuf> {
    content
        .lines()
        .filter_map(|line| {
            // Each path line looks like:   "path"   "/some/path"
            let parts: Vec<&str> = line.split('"').collect();
            if parts.len() >= 4 && parts[1] == "path" {
                Some(PathBuf::from(parts[3]))
            } else {
                None
            }
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn find_steam_exe() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let known_paths = [
        PathBuf::from("/usr/bin/steam"),
        PathBuf::from("/usr/games/steam"),
        PathBuf::from("/snap/bin/steam"),
    ];
    for path in &known_paths {
        if path.exists() {
            return Ok(path.clone());
        }
    }
    // Check steam.sh inside the Steam installation
    if let Ok(install) = find_steam_installation() {
        let steam_sh = install.join("steam.sh");
        if steam_sh.exists() {
            return Ok(steam_sh);
        }
    }
    // Fall back to PATH
    let output = Command::new("which").arg("steam").output()?;
    if output.status.success() {
        let path_str = String::from_utf8(output.stdout)?.trim().to_string();
        return Ok(PathBuf::from(path_str));
    }
    Err(
        "Steam executable not found. Install Steam from https://store.steampowered.com/about/"
            .into(),
    )
}

#[cfg(target_os = "linux")]
fn run_replay(replay_name: String) -> Result<(), Box<dyn std::error::Error>> {
    let steam_exe = find_steam_exe()?;
    println!("Steam located at {}", steam_exe.display());
    println!("Starting AOE4...");
    Command::new(steam_exe)
        .args([
            "-applaunch",
            AOE4_APP_ID,
            "-dev",
            "-replay",
            &format!("playback:{}", replay_name),
        ])
        // Don't wait: when Steam isn't running yet, this process is the Steam client itself.
        .spawn()
        .map_err(|e| format!("Failed to launch Steam: {e}"))?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn register_url_protocol_at(exe_path: &std::path::Path) -> std::io::Result<()> {
    let exe_str = exe_path
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Non-UTF8 exe path"))?;

    let desktop_content = format!(
        "[Desktop Entry]\nName=AOE4 Replay Launcher\nExec=\"{exe_str}\" %u\nType=Application\nTerminal=true\nNoDisplay=true\nMimeType=x-scheme-handler/aoe4rep;\n"
    );

    let home = dirs::home_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Home directory not found"))?;
    let apps_dir = home.join(".local/share/applications");
    fs::create_dir_all(&apps_dir)?;
    let desktop_path = apps_dir.join("aoe4rep-handler.desktop");
    fs::write(&desktop_path, &desktop_content)?;

    let status = Command::new("xdg-mime")
        .args([
            "default",
            "aoe4rep-handler.desktop",
            "x-scheme-handler/aoe4rep",
        ])
        .status()?;
    if !status.success() {
        return Err(io::Error::other("xdg-mime failed to register aoe4rep"));
    }

    // Non-fatal: updates MIME cache so the handler is picked up immediately
    Command::new("update-desktop-database")
        .arg(&apps_dir)
        .status()
        .ok();

    println!("Protocol 'aoe4rep' registered successfully for the current user!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_url_preserves_original_url_as_one_query_parameter() {
        for original in [
            "aoe4rep://m/123456",
            "aoe4rep://replay/opaque-id?token=a+b%2Fc==&expires=123#fragment",
        ] {
            let url = replay_download_url(original).unwrap();
            assert_eq!(url.origin().ascii_serialization(), HOME_URL);
            assert_eq!(url.path(), "/api/replays");
            assert_eq!(url.fragment(), None);
            assert_eq!(
                url.query_pairs().into_owned().collect::<Vec<_>>(),
                vec![("url".to_string(), original.to_string())]
            );
        }
    }

    #[test]
    fn user_agent_carries_launcher_version() {
        assert_eq!(
            USER_AGENT,
            format!("aoe4_replay_launcher/{}", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn vdf_parser_extracts_all_paths() {
        let vdf = r#""libraryfolders"
{
    "0"
    {
        "path"      "/home/user/.steam/debian-installation"
        "apps"      {}
    }
    "1"
    {
        "path"      "/mnt/storage/SteamLibrary"
        "apps"      {}
    }
}"#;
        let paths = parse_library_paths_from_vdf(vdf);
        assert_eq!(paths.len(), 2);
        assert_eq!(
            paths[0],
            PathBuf::from("/home/user/.steam/debian-installation")
        );
        assert_eq!(paths[1], PathBuf::from("/mnt/storage/SteamLibrary"));
    }

    #[test]
    fn vdf_parser_ignores_non_path_keys() {
        let vdf = r#""label"   "my games"
"path"    "/mnt/games"
"totalsize" "1000""#;
        let paths = parse_library_paths_from_vdf(vdf);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], PathBuf::from("/mnt/games"));
    }
}
