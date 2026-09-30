//! Game save, restore, and shell-escape handling.
//!
//! Ported from `src/c/save.c` to Rust. Save/restore files are read and written
//! with the Rust standard library (`std::fs`, `std::io`) rather than C stdio.
use crate::machdep::setup;
use crate::options::read_line;
use crate::rnd::set_seed;
use crate::startup::playit;
use crate::state::{rs_restore_file, rs_save_file};
use crate::ui::input::{self, readchar};
use crate::ui::output::{self, msg_str};
use crate::ui::terminal;
use std::fs::File;
use std::io::{Read, Write};

use std::path::Path;

const ESCAPE: i32 = 27;

/// Magic header that identifies a RON save file written by this version.
pub const RON_MAGIC: &[u8] = b"ROGUE-RON 1\n";

use crate::game::globals::{master_mode_enabled, mpos, wizard};

/// Checks the restored player state and reports whether the saved game is already dead.
unsafe fn restore_player_dead() -> bool {
    crate::game::PLAYER.stats().hit_points <= 0
}

/// Implements the interactive save command flow and then delegates the actual write to save_file.
pub unsafe fn save_game() {
    let mut c: i32;
    let mut buf = crate::game::globals::file_name();

    mpos = 0;

    'over: loop {
        if !crate::game::globals::file_name().is_empty() {
            loop {
                msg_str(&format!(
                    "save file ({})? ",
                    crate::game::globals::file_name()
                ));
                c = readchar();
                mpos = 0;
                if c == ESCAPE {
                    msg_str("");
                    return;
                }
                if c == 'n' as i32 || c == 'N' as i32 || c == 'y' as i32 || c == 'Y' as i32 {
                    break;
                }
                msg_str("please answer Y or N");
            }

            if c == 'y' as i32 || c == 'Y' as i32 {
                crate::ui::terminal::UI.write_text("Yes\n");
                output::refresh();
                buf = crate::game::globals::file_name();
            } else {
                buf = String::new();
            }
        } else {
            buf = String::new();
        }

        loop {
            if buf.is_empty() {
                mpos = 0;
                msg_str("file name: ");
                match read_line("") {
                    None => {
                        msg_str("");
                        return;
                    }
                    Some(text) => buf = text,
                }
                mpos = 0;
            }

            if Path::new(&buf).exists() {
                loop {
                    msg_str("File exists.  Do you wish to overwrite it?");
                    mpos = 0;
                    c = readchar();
                    if c == ESCAPE {
                        msg_str("");
                        return;
                    }
                    if c == 'y' as i32 || c == 'Y' as i32 {
                        break;
                    }
                    if c == 'n' as i32 || c == 'N' as i32 {
                        continue 'over;
                    }
                    msg_str("Please answer Y or N");
                }
                msg_str(&format!("file name: {}", buf));
                let _ = std::fs::remove_file(&buf);
            }

            crate::game::globals::set_file_name(buf.clone());
            match File::create(&buf) {
                Ok(mut savef) => save_file(&mut savef),
                Err(err) => {
                    msg_str(&format!("error {}", err));
                    buf = String::new();
                }
            }
        }
    }
}

/// Writes the RON save-file header and hands off the actual save payload to the
/// state serializer.
pub unsafe fn save_file(savef: &mut File) {
    let _ = std::io::stdout().write_all(b"\n");
    output::flush_now();
    terminal::UI.deinit_terminal();
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(
        crate::game::globals::file_name(),
        std::fs::Permissions::from_mode(0o400),
    );

    let _ = savef.write_all(RON_MAGIC);

    let _ = rs_save_file(savef);
    let _ = savef.flush();
    std::process::exit(0);
}

/// Restores a saved game from disk, rebuilds runtime state, and resumes the main game loop.
///
/// Save files are RON documents preceded by a short [`RON_MAGIC`] header line.
pub unsafe fn restore(file: &str) -> u8 {
    let mut magic = [0u8; 12];

    // The caller passes a file argument (typically "-r").
    let mut file_name = file.to_string();

    if file_name == "-r" {
        file_name = crate::game::globals::file_name();
    }

    #[cfg(unix)]
    libc::signal(libc::SIGTSTP, libc::SIG_IGN);

    let mut inf = match File::open(&file_name) {
        Ok(f) => f,
        Err(_) => {
            msg_str(&format!("{}: cannot open", file_name));
            return 0;
        }
    };

    if inf.read_exact(&mut magic[..RON_MAGIC.len()]).is_err()
        || magic[..RON_MAGIC.len()] != *RON_MAGIC
    {
        msg_str("Sorry, saved game is out of date.\n");
        return 0;
    }

    setup();
    if let Err(err) = rs_restore_file(&mut inf) {
        output::flush_now();
        terminal::UI.deinit_terminal();
        msg_str(&format!("Sorry, saved game could not be read: {}", err));
        return 0;
    }

    if (master_mode_enabled == 0 || wizard == 0) && std::fs::remove_file(&file_name).is_err() {
        msg_str("Cannot unlink file\n");
        return 0;
    }

    mpos = 0;

    if restore_player_dead() {
        output::flush_now();
        terminal::UI.deinit_terminal();
        msg_str("\n\"He's dead, Jim\"\n");
        return 0;
    }

    #[cfg(unix)]
    libc::signal(libc::SIGTSTP, crate::startup::tstp as libc::sighandler_t);
    crate::game::globals::set_file_name(file_name.clone());
    set_seed(std::process::id() as i32);
    msg_str(&format!("file name: {}", file_name));
    playit();
    0
}

/// Handles signal-triggered autosave by reopening the current save file and delegating to save_file.
pub unsafe extern "C" fn auto_save(sig: i32) {
    let _ = sig;

    #[cfg(unix)]
    for signal in 0..32 {
        libc::signal(signal, libc::SIG_IGN);
    }
    let file_name = crate::game::globals::file_name();
    if !file_name.is_empty() {
        match File::create(&file_name) {
            Ok(mut savef) => save_file(&mut savef),
            Err(_) => {
                if std::fs::remove_file(&file_name).is_ok() {
                    if let Ok(mut savef) = File::create(&file_name) {
                        save_file(&mut savef);
                    }
                }
            }
        }
    }
    std::process::exit(0);
}
