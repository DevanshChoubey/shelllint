//! Out-of-memory behaviour of the Haskell binary: GHC's RTS prints
//! `<prog>: out of memory` and exits 251, where Rust would abort with
//! SIGABRT (exit 134). `std::alloc::set_alloc_error_hook` is unstable, so a
//! global allocator wrapper reports a failed allocation itself.

use std::alloc::{GlobalAlloc, Layout, System};

pub const OOM_MESSAGE: &[u8] = b"shellcheck-rust: out of memory\n";
pub const OOM_EXIT_CODE: i32 = 251;

pub struct ExitOnOom;

#[cfg(unix)]
#[cold]
fn die() -> ! {
    // Raw write and _exit: nothing here may allocate.
    unsafe {
        libc::write(2, OOM_MESSAGE.as_ptr().cast(), OOM_MESSAGE.len());
        libc::_exit(OOM_EXIT_CODE)
    }
}

#[cfg(not(unix))]
#[cold]
fn die() -> ! {
    use std::io::Write;
    let _ = std::io::stderr().write_all(OOM_MESSAGE);
    std::process::exit(OOM_EXIT_CODE)
}

#[inline]
fn check(p: *mut u8) -> *mut u8 {
    if p.is_null() {
        die()
    }
    p
}

// SAFETY: every method forwards to `System` unchanged; a null result never
// reaches the caller because the process exits first.
unsafe impl GlobalAlloc for ExitOnOom {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        check(unsafe { System.alloc(layout) })
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        check(unsafe { System.alloc_zeroed(layout) })
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        check(unsafe { System.realloc(ptr, layout, new_size) })
    }
}
