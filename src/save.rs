//! Game save, restore, and shell-escape handling.
//!
//! Ported from `src/c/save.c` to Rust. Save/restore files are read and written
//! with the Rust standard library (`std::fs`, `std::io`) rather than C stdio.
use crate::machdep::setup;
use crate::options::read_line;
use crate::rnd::set_seed;
use crate::startup::{main_loop_step, request_exit};
use crate::state::{rs_restore_file, rs_save_file};
use crate::ui::input::{self, readchar};
use crate::ui::output::{self, msg_str};
use crate::ui::terminal;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Write};

use std::path::{Path, PathBuf};

const ESCAPE: i32 = 27;

/// Magic header that identifies a RON save file written by this version.
pub const RON_MAGIC: &[u8] = b"ROGUE-RON 1\n";

#[derive(Debug)]
pub enum RestoreError {
    Open { path: PathBuf, source: io::Error },
    ReadHeader { path: PathBuf, source: io::Error },
    InvalidHeader { path: PathBuf },
    ReadState { path: PathBuf, source: io::Error },
    RemoveFile { path: PathBuf, source: io::Error },
    DeadPlayer { path: PathBuf },
}

impl fmt::Display for RestoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open { path, source } => {
                write!(
                    formatter,
                    "{}: could not open save file: {source}",
                    path.display()
                )
            }
            Self::ReadHeader { path, source } => {
                write!(
                    formatter,
                    "{}: could not read save header: {source}",
                    path.display()
                )
            }
            Self::InvalidHeader { path } => {
                write!(formatter, "{}: save file is out of date", path.display())
            }
            Self::ReadState { path, source } => {
                write!(
                    formatter,
                    "{}: could not restore save data: {source}",
                    path.display()
                )
            }
            Self::RemoveFile { path, source } => {
                write!(
                    formatter,
                    "{}: could not remove save file: {source}",
                    path.display()
                )
            }
            Self::DeadPlayer { path } => {
                write!(formatter, "{}: save contains a dead player", path.display())
            }
        }
    }
}

impl Error for RestoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Open { source, .. }
            | Self::ReadHeader { source, .. }
            | Self::ReadState { source, .. }
            | Self::RemoveFile { source, .. } => Some(source),
            Self::InvalidHeader { .. } | Self::DeadPlayer { .. } => None,
        }
    }
}

use crate::game::globals::{master_mode_enabled, mpos, wizard};

/// Checks the restored player state and reports whether the saved game is already dead.
unsafe fn restore_player_dead() -> bool {
    crate::game::PLAYER.stats().hit_points <= 0
}

/// Implements the interactive save command flow and then delegates the actual write to save_file.
pub unsafe fn save_game() {
    let mut c: i32;
    let mut buf: String;

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
                if let Err(error) = std::fs::remove_file(&buf) {
                    msg_str(&format!("could not remove save file: {error}"));
                    continue 'over;
                }
            }

            crate::game::globals::set_file_name(buf.clone());
            match File::create(&buf) {
                Ok(mut savef) => match save_file(&mut savef) {
                    Ok(()) => request_exit(0),
                    Err(error) => {
                        input::enable_raw_mode();
                        msg_str(&format!("could not save game: {error}"));
                        buf = String::new();
                    }
                },
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
pub unsafe fn save_file(savef: &mut File) -> io::Result<()> {
    let _ = std::io::stdout().write_all(b"\n");
    output::flush_now();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(
        crate::game::globals::file_name(),
        std::fs::Permissions::from_mode(0o400),
    )?;

    write_save_data(savef)
}

fn write_save_data(savef: &mut dyn Write) -> io::Result<()> {
    savef.write_all(RON_MAGIC)?;
    rs_save_file(savef)?;
    savef.flush()
}

/// Restores a saved game from disk, rebuilds runtime state, and resumes the main game loop.
///
/// Save files are RON documents preceded by a short [`RON_MAGIC`] header line.
pub unsafe fn restore(file: &str) -> Result<(), RestoreError> {
    let mut magic = [0u8; 12];

    // The caller passes a file argument (typically "-r").
    let mut file_name = file.to_string();

    if file_name == "-r" {
        file_name = crate::game::globals::file_name();
    }

    let path = PathBuf::from(&file_name);
    let mut inf = File::open(&path).map_err(|source| RestoreError::Open {
        path: path.clone(),
        source,
    })?;

    inf.read_exact(&mut magic[..RON_MAGIC.len()])
        .map_err(|source| RestoreError::ReadHeader {
            path: path.clone(),
            source,
        })?;
    if magic[..RON_MAGIC.len()] != *RON_MAGIC {
        return Err(RestoreError::InvalidHeader { path });
    }

    setup();
    if let Err(source) = rs_restore_file(&mut inf) {
        output::flush_now();
        return Err(RestoreError::ReadState {
            path: path.clone(),
            source,
        });
    }

    if master_mode_enabled == 0 || wizard == 0 {
        if let Err(source) = std::fs::remove_file(&path) {
            output::flush_now();
            return Err(RestoreError::RemoveFile {
                path: path.clone(),
                source,
            });
        }
    }

    mpos = 0;

    if restore_player_dead() {
        output::flush_now();
        return Err(RestoreError::DeadPlayer { path });
    }

    crate::game::globals::set_file_name(file_name.clone());
    set_seed(std::process::id() as i32);
    msg_str(&format!("file name: {}", file_name));
    main_loop_step();
    Ok(())
}

/// Handles signal-triggered autosave by reopening the current save file and delegating to save_file.
pub unsafe fn auto_save() -> io::Result<()> {
    #[cfg(unix)]
    for signal in 0..32 {
        libc::signal(signal, libc::SIG_IGN);
    }
    let file_name = crate::game::globals::file_name();
    if !file_name.is_empty() {
        let save_result = match File::create(&file_name) {
            Ok(mut savef) => save_file(&mut savef),
            Err(create_error) => match std::fs::remove_file(&file_name) {
                Ok(()) => File::create(&file_name).and_then(|mut savef| save_file(&mut savef)),
                Err(_) => Err(create_error),
            },
        };
        save_result?;
    }
    request_exit(0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{restore, write_save_data, RestoreError};
    use std::io::{self, Write};
    use std::path::PathBuf;

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn test_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("rogue-{name}-{}", std::process::id()))
    }

    #[test]
    fn restore_returns_open_error_with_path() {
        let path = test_path("missing-save");
        let _ = std::fs::remove_file(&path);

        let error = unsafe { restore(path.to_str().unwrap()) }.unwrap_err();
        assert!(matches!(error, RestoreError::Open { .. }));
        assert!(error.to_string().contains(&path.display().to_string()));
    }

    #[test]
    fn restore_returns_read_error_for_truncated_header() {
        let path = test_path("truncated-save");
        std::fs::write(&path, []).unwrap();

        let error = unsafe { restore(path.to_str().unwrap()) }.unwrap_err();
        assert!(matches!(error, RestoreError::ReadHeader { .. }));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn restore_returns_invalid_header_error() {
        let path = test_path("invalid-save");
        std::fs::write(&path, b"INVALID HEADER\n").unwrap();

        let error = unsafe { restore(path.to_str().unwrap()) }.unwrap_err();
        assert!(matches!(error, RestoreError::InvalidHeader { .. }));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn save_returns_writer_errors() {
        let error = write_save_data(&mut FailingWriter).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    }
}
