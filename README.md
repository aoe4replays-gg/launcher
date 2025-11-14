# AOE4 replay launcher

A launcher that allows replays from https://aoe4replays.gg to be run automatically in a single click from the website.

Currently only works on Windows and if AOE4 is installed through Steam.

[Download link (includes instructions)](https://github.com/aoe4replays-gg/launcher/raw/refs/heads/main/aoe4_replay_launcher.zip)


## How it works
When manually run, the launcher binds itself as handler of the `aoe4rep://` URL protocol in Windows (a custom URL format we defined for this use-case).

Then when the user clicks a replay URL on aoe4replays.gg, the launcher : 
- gets triggered by Windows and receives the replay URL,
- extracts the ID of the match from the URL,
- downloads the corresponding replay file from aoe4replays.gg,
- unzips it into the local aoe4 playback folder,
- starts AOE4 in replay mode through Steam, with appropriate arguments so that it immediately runs the replay.

## Building the project from sources
- be on a windows machine with Rust installed
- clone this github repository
- run `cargo build --release` in the root folder of the repository
- the executable is gets generated at `target/release/aoe4_replay_launcher.exe`

