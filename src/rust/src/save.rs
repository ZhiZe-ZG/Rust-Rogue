use std::ffi::CStr;
use std::os::raw::{c_char, c_int};
use std::ptr;

#[repr(C)]
pub struct CFile {
    _private: [u8; 0],
}

extern "C" {
    static mut LINES: c_int;
    static mut COLS: c_int;
    static mut file_name: c_char;
    static mut version: c_char;

    fn mvcur(ly: c_int, lx: c_int, y: c_int, x: c_int) -> c_int;
    fn putchar(c: c_int) -> c_int;
    fn endwin() -> c_int;
    fn resetltchars();
    fn md_chmod(filename: *mut c_char, mode: c_int) -> c_int;
    fn fopen(path: *const c_char, mode: *const c_char) -> *mut CFile;
    fn md_ignoreallsignals();
    fn md_unlink_open_file(file: *mut c_char, inf: *mut CFile) -> c_int;
    fn fwrite(ptr: *const u8, size: usize, nmemb: usize, stream: *mut CFile) -> usize;
    fn rs_save_file(savef: *mut CFile);
    fn fflush(stream: *mut CFile) -> c_int;
    fn fclose(stream: *mut CFile) -> c_int;
    fn exit(status: c_int) -> !;
}

/// Writes the save-file header and hands off the actual save payload to the existing C helpers.
#[no_mangle]
pub unsafe extern "C" fn save_file(savef: *mut CFile) {
    let mut buf = [0u8; 80];
    let lines = LINES;
    let cols = COLS;
    let header = format!("{} x {}\n", lines, cols);
    let version_ptr = &raw const version;

    mvcur(0, cols - 1, lines - 1, 0);
    putchar('\n' as c_int);
    endwin();
    resetltchars();
    md_chmod(&raw mut file_name as *mut c_char, 0o400);

    fwrite(
        version_ptr as *const u8,
        1,
        CStr::from_ptr(version_ptr).to_bytes_with_nul().len(),
        savef,
    );

    buf[..header.len()].copy_from_slice(header.as_bytes());
    fwrite(buf.as_ptr(), 1, buf.len(), savef);

    rs_save_file(savef);
    fflush(savef);
    fclose(savef);
    exit(0)
}

/// Handles signal-triggered autosave by reusing the regular save-file writer.
#[no_mangle]
pub unsafe extern "C" fn auto_save(sig: c_int) {
    let _ = sig;
    let file_name_ptr = &raw mut file_name as *mut c_char;
    let mode = c"w";
    let mut savef: *mut CFile;

    md_ignoreallsignals();
    if ptr::read(file_name_ptr) != 0
        && ((!{
            savef = fopen(file_name_ptr, mode.as_ptr());
            savef.is_null()
        }) || (md_unlink_open_file(file_name_ptr, savef) >= 0 && !{
            savef = fopen(file_name_ptr, mode.as_ptr());
            savef.is_null()
        }))
    {
        save_file(savef);
    }
    exit(0)
}
