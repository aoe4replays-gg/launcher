The launcher that allows replays from https://aoe4replays.gg to be run automatically in a single click.

The launcher binds itself as handler of the "aoe4rep://" URL protocol in Windows, then once triggered it downloads the replay file and starts AOE4 in replay mode.

Build instructions :
- install Rust
- run `cargo build --release`
- the runnable gets generated in target/release/aoe4_replay_launcher.exe
