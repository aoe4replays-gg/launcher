# AOE4 replay launcher

A launcher that allows replays from https://aoe4replays.gg to be run automatically in a single click from the website.

Currently only works on Windows and if AOE4 is installed through Steam.

Download links (with instructions) :
- [Windows launcher v1.1](https://github.com/aoe4replays-gg/launcher/releases/download/V1.1/aoe4-replay-launcher-windows-x86_64.zip)
- [Linux launcher v1.1](https://github.com/aoe4replays-gg/launcher/releases/download/V1.1/aoe4-replay-launcher-linux-x86_64.zip)


## How it works
When manually run, the launcher binds itself as handler of the `aoe4rep://` URL protocol in Windows (a custom URL format we defined for this use-case).

Then when the user clicks a replay URL on aoe4replays.gg, the launcher : 
- gets triggered by Windows and receives the replay URL,
- extracts the ID of the match from the URL,
- downloads the corresponding replay file from aoe4replays.gg,
- unzips it into the local AOE4 playback folder,
- starts AOE4 in dev mode through Steam, with appropriate arguments so that it immediately runs the replay.

## Building the launcher from the sources
- be on a Windows machine with Rust installed,
- clone this github repository,
- run `cargo build --release` at the root folder of the repository,
- the executable gets generated in `target/release/aoe4_replay_launcher.exe`

