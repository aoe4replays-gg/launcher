use std::io::{self, BufRead, Write};

use ::windows::Win32::System::Com::{
    CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use ::windows::Win32::UI::Shell::{
    AO_NONE, ApplicationActivationManager, IApplicationActivationManager,
};
use ::windows::core::{HSTRING, w};

// Unflagged URLs retain the original Steam behavior for existing URL registrations.
pub fn parse_args(args: &[String]) -> Result<(bool, Option<&str>), &'static str> {
    match args {
        [] => Ok((false, None)),
        [url] if !url.starts_with('-') => Ok((false, Some(url))),
        [flag, url] if flag == "--steam" && !url.starts_with('-') => Ok((false, Some(url))),
        [flag, url] if flag == "--xbox" && !url.starts_with('-') => Ok((true, Some(url))),
        _ => Err("Run without arguments for setup, or use [--steam | --xbox] <replay URL>."),
    }
}

pub fn choose_platform(input: &mut impl BufRead, output: &mut impl Write) -> io::Result<bool> {
    writeln!(
        output,
        "Choose your Age of Empires IV edition:\n1) Steam\n2) Xbox / Microsoft Store"
    )?;
    loop {
        write!(output, "Enter 1 or 2: ")?;
        output.flush()?;
        let mut answer = String::new();
        if input.read_line(&mut answer)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Setup cancelled: no edition selected.",
            ));
        }
        match answer.trim() {
            "1" => return Ok(false),
            "2" => return Ok(true),
            _ => writeln!(output, "Please enter 1 or 2.")?,
        }
    }
}

pub fn protocol_command(exe: &str, xbox: bool) -> String {
    let flag = if xbox { "--xbox" } else { "--steam" };
    format!("\"{exe}\" {flag} \"%1\"")
}

pub fn launch_xbox(replay_name: &str) -> ::windows::core::Result<()> {
    // The replay name is generated locally, never supplied by the replay URL.
    let arguments = HSTRING::from(format!("-dev -replay playback:{replay_name}"));
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
    }
    struct ComApartment;
    impl Drop for ComApartment {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }
    let _apartment = ComApartment;
    // Release the manager before uninitializing COM. The Xbox helper handles
    // package activation/licensing and forwards the arguments to the game.
    let manager: IApplicationActivationManager =
        unsafe { CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_LOCAL_SERVER)? };
    println!("Starting AOE4 through Xbox / Microsoft Store...");
    unsafe {
        manager.ActivateApplication(
            w!("Microsoft.Cardinal_8wekyb3d8bbwe!Game"),
            &arguments,
            AO_NONE,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_and_legacy_urls_are_supported() {
        assert_eq!(parse_args(&[]).unwrap(), (false, None));
        let args = ["aoe4rep://m/123".into()];
        assert_eq!(parse_args(&args).unwrap(), (false, Some(args[0].as_str())));
    }

    #[test]
    fn explicit_flags_preserve_the_entire_url() {
        for (flag, xbox) in [("--steam", false), ("--xbox", true)] {
            let args = [
                flag.into(),
                "aoe4rep://replay/id?token=a+b%2Fc==&expires=123#fragment".into(),
            ];
            assert_eq!(parse_args(&args).unwrap(), (xbox, Some(args[1].as_str())));
        }
    }

    #[test]
    fn rejects_missing_urls_unknown_flags_and_extra_arguments() {
        for args in [
            vec!["--xbox"],
            vec!["--steam"],
            vec!["--unknown", "url"],
            vec!["--xbox", "url", "extra"],
            vec!["--xbox", "--steam"],
        ] {
            let args: Vec<_> = args.into_iter().map(String::from).collect();
            assert!(parse_args(&args).is_err());
        }
    }

    #[test]
    fn setup_retries_invalid_choices_and_handles_closed_input() {
        let mut output = Vec::new();
        assert!(choose_platform(&mut io::Cursor::new(b"invalid\n\n2\n"), &mut output).unwrap());
        assert!(!choose_platform(&mut io::Cursor::new(b"1\n"), &mut Vec::new()).unwrap());
        assert_eq!(
            choose_platform(&mut io::Cursor::new(b""), &mut Vec::new())
                .unwrap_err()
                .kind(),
            io::ErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn handler_quotes_paths_and_urls_and_stores_the_selected_flag() {
        for (xbox, flag) in [(false, "--steam"), (true, "--xbox")] {
            assert_eq!(
                protocol_command(r"C:\My Apps\launcher.exe", xbox),
                format!("\"C:\\My Apps\\launcher.exe\" {flag} \"%1\"")
            );
        }
    }
}
