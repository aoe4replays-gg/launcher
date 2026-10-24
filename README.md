# AOE4 replay launcher

A launcher that allows replays from https://aoe4replays.gg to be run in a single click from the website.

Currently only works with Steam (no Gamepass support).

Download links (with instructions) in https://github.com/aoe4replays-gg/launcher/releases

## How it works
When manually run, the launcher binds itself as handler of the `aoe4rep://` URL protocol in Windows (a custom URL format we defined for this use-case).

Then when the user clicks a replay URL on aoe4replays.gg, the launcher : 
- gets triggered by Windows and receives the replay URL,
- passes the original URL to `https://aoe4replays.gg/api/replays` as the encoded `url` query parameter,
- downloads the corresponding replay file from aoe4replays.gg using a generated local filename,
- unzips it into the local AOE4 playback folder,
- starts AOE4 in dev mode through Steam, with appropriate arguments so that it immediately runs the replay.

## Building the launcher from the sources
- be on a Windows or Linux machine with Rust installed,
- clone this github repository,
- run `cargo build --release` at the root folder of the repository,
- the executable gets generated in `target/release/aoe4_replay_launcher.exe`

A release build hands over to the installed launcher unless its version is newer, so use `cargo run` (a debug build, which always runs its own code) while developing.

Releases are published by pushing a lowercase tag matching the Cargo version, e.g. `v2.0.0` (the full three-part version, not `v2.0`). Tags with a prerelease suffix (`v2.1.0-rc.1`) are published as GitHub prereleases and are not installed automatically.
