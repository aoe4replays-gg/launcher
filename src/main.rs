use std::env;
use std::fs::File;
use std::io::{self, copy, Cursor};
use std::path::PathBuf;
use std::process::Command;

use dirs::document_dir;
use flate2::read::GzDecoder;
use reqwest::blocking::get;
use winreg::enums::*;
use winreg::RegKey;

const HOME_URL: &str = "https://aoe4replays.gg";
const PLAYBACK_PATH: &str = "My Games\\Age of Empires IV\\playback";

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        let url = (&args[1]).to_string();
        println!("URL = {url}");
        let match_id = parse_aoe4rep_url(url)
            .expect("Failed to parse URL");
        println!("Match ID = {match_id}");
        let replay_name = download_replay(match_id)
            .expect("Failed to get replay");
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

fn download_replay(match_id: u64) -> Result<String, Box<dyn std::error::Error>> {
    let filename = format!("AgeIV_Replay_{}", match_id);
    let mut folder = document_dir().ok_or("Could not find Documents directory")?;
    folder.push(PLAYBACK_PATH);
    println!("Replay playback folder detected in : {}", folder.display());
    let mut file_path = folder.clone();
    file_path.push(&filename);
    let url = format!("{}/api/replays/{}", HOME_URL, match_id);
    let bytes = get(url)?.bytes()?;
    let cursor = Cursor::new(bytes);
    let mut decoder = GzDecoder::new(cursor);
    let mut output = File::create(file_path)?;
    copy(&mut decoder, &mut output)?;
    println!("Downloaded replay file {}", filename);
    Ok(filename)
}

fn run_replay(replay_name: String) {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let steam_key = hkcu.open_subkey("Software\\Valve\\Steam").expect("Steam entry not found in registry");
    let steam_exe: String = steam_key.get_value("SteamExe").expect("SteamExe entry not found in registry");
    println!("Steam located in {steam_exe}");
    println!("Starting AOE4...");
    Command::new(steam_exe)
        .args([
            "-applaunch",
            "1466860",
            "-dev",
            "-replay",
            &format!("playback:{}", replay_name),
        ])
        .status()
        .expect("Failed to launch command");
}

fn register_url_protocol() -> std::io::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey("Software\\Classes\\aoe4rep")?;
    key.set_value("", &"URL:AOE4Rep Protocol")?;
    key.set_value("URL Protocol", &"")?;
    let (command_key, _) = key.create_subkey(r"shell\open\command")?;
    let exe_path: PathBuf = env::current_exe()?;
    command_key.set_value("", &format!("\"{}\" \"%1\"", exe_path.to_str().unwrap()))?;
    println!("Protocol 'aoe4rep' registered successfully for the current user!");
    Ok(())
}

fn parse_aoe4rep_url(url: String) -> Result<u64, String> {
    let prefix = "aoe4rep://m/";
    if !url.starts_with(prefix) {
        return Err(format!("URL does not start with '{}'", prefix));
    }
    let match_id = url.strip_prefix(prefix).expect("Failed to get matchId");
    let a = match_id.parse::<u64>().map_err(|_| format!("Failed to parse matchId '{}' as a number", match_id))?;
    Ok(a)
}
