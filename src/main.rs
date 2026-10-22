use std::env;
#[cfg(target_os = "linux")]
use std::fs;
use std::fs::File;
use std::io::{self, copy, Cursor};
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;
use reqwest::blocking::get;

#[cfg(windows)]
use winreg::enums::*;
#[cfg(windows)]
use winreg::RegKey;

#[cfg(windows)]
mod windows;

const HOME_URL: &str = "https://aoe4replays.gg";
const AOE4_APP_ID: &str = "1466860";

fn main() {
    let args: Vec<String> = env::args().collect();
    #[cfg(windows)]
    let (xbox, url) = match windows::parse_args(&args[1..]) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("{error}");
            wait_for_key();
            std::process::exit(1);
        }
    };
    #[cfg(not(windows))]
    let url = args.get(1).map(String::as_str);
    if let Some(url) = url {
        println!("URL = {url}");
        let replay_name = match download_replay(url) {
            Ok(replay_name) => replay_name,
            Err(error) => {
                eprintln!("Failed to download replay: {error}");
                wait_for_key();
                std::process::exit(1);
            }
        };
        #[cfg(windows)]
        run_replay(replay_name, xbox);
        #[cfg(not(windows))]
        run_replay(replay_name);
    } else {
        println!("Configuring...");
        if let Err(e) = register_url_protocol() {
            eprintln!("Failed to configure aoe4rep protocol: {}", e);
        } else {
            println!("Configuration done, you can now launch replays from {HOME_URL}");
        }
        wait_for_key();
    }
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
    let response = get(replay_download_url(url)?)?;
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
fn run_replay(replay_name: String, xbox: bool) {
    if xbox {
        if let Err(error) = windows::launch_xbox(&replay_name) {
            eprintln!("Failed to launch Xbox edition: {error}");
            wait_for_key();
            std::process::exit(1);
        }
        return;
    }
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let steam_key = hkcu
        .open_subkey("Software\\Valve\\Steam")
        .expect("Steam entry not found in registry");
    let steam_exe: String = steam_key
        .get_value("SteamExe")
        .expect("SteamExe entry not found in registry");
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
        .status()
        .expect("Failed to launch command");
}

#[cfg(windows)]
fn register_url_protocol() -> std::io::Result<()> {
    let xbox = windows::choose_platform(&mut io::stdin().lock(), &mut io::stdout().lock())?;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey("Software\\Classes\\aoe4rep")?;
    key.set_value("", &"URL:AOE4Rep Protocol")?;
    key.set_value("URL Protocol", &"")?;
    let (command_key, _) = key.create_subkey(r"shell\open\command")?;
    let exe_path: PathBuf = env::current_exe()?;
    command_key.set_value("", &windows::protocol_command(exe_path.to_str().unwrap(), xbox))?;
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
    Err("Steam executable not found. Install Steam from https://store.steampowered.com/about/".into())
}

#[cfg(target_os = "linux")]
fn run_replay(replay_name: String) {
    let steam_exe = find_steam_exe().expect("Steam not found");
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
        .status()
        .expect("Failed to launch Steam");
}

#[cfg(target_os = "linux")]
fn register_url_protocol() -> std::io::Result<()> {
    let exe_path = env::current_exe()?;
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

    Command::new("xdg-mime")
        .args(["default", "aoe4rep-handler.desktop", "x-scheme-handler/aoe4rep"])
        .status()?;

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
        assert_eq!(paths[0], PathBuf::from("/home/user/.steam/debian-installation"));
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
