# AOE4 replay launcher

A launcher that allows replays from https://aoe4replays.gg to be run automatically in a single click from the website.
Currently only supports players using Steam on Windows.

[Download link (includes instructions)](/aoe4_replay_launcher.zip)


## How it works
When manually run, the launcher binds itself as handler of the `aoe4rep://` URL protocol in Windows (a custom URL format we defined for this specific use-case).

Then when the user clicks a replay URL on the site, the launcher : 
- gets triggered by Windows and receives the replay URL as parameter,
- extracts the ID of the match from the URL,
- downloads the corresponding replay file from aoe4replays.gg,
- unzips it into the local aoe4 playback folder,
- starts AOE4 in replay mode through Steam, with appropriate arguments so that it immediatly runs the replay.

## Build from sources instructions
- be on a windows machine with Rust installed
- clone this github repository
- run `cargo build --release` in the root folder of the repository
- the executable is gets generated at `target/release/aoe4_replay_launcher.exe`

