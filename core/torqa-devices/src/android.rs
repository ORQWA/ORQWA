//! Bluetooth on Android (ADR 0024): btleplug reaches Android's Bluetooth through Java classes
//! packed into the APK, and finds them only through the Java environment of a thread that
//! Android started with the app's class loader. That thread is the one loading Torqa's library,
//! so the library's `JNI_OnLoad` (in torqa-gd) hands its Java VM to [`init`].
#![allow(unsafe_code)] // JNI and Android's log are C interfaces; each use says why it is sound

use std::ffi::{CString, c_char, c_int};

use jni::JavaVM;

/// The Java VM's types, for the library's entry point.
pub use jni::sys;

/// Readies Bluetooth for scans and connections, from the thread that loads the library.
///
/// # Errors
/// What btleplug reports, e.g. when its Java classes are missing from the APK.
///
/// # Safety
/// `vm` must be the Java VM Android passes to `JNI_OnLoad`.
pub unsafe fn init(vm: *mut sys::JavaVM) -> Result<(), String> {
    // SAFETY: the caller passes the VM that Android handed to JNI_OnLoad.
    let vm = unsafe { JavaVM::from_raw(vm) };
    vm.attach_current_thread(|env| -> Result<(), Box<dyn std::error::Error>> {
        btleplug::platform::init(env)?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(priority: c_int, tag: *const c_char, text: *const c_char) -> c_int;
}

/// Writes `message` to Android's log (`adb logcat -s Torqa`), where nothing else reaches before
/// Godot's own log is up.
pub fn log(message: &str) {
    /// Android's `ANDROID_LOG_INFO`.
    const INFO: c_int = 4;
    let text = CString::new(message.replace('\0', " ")).unwrap_or_default();
    // SAFETY: both strings are NUL-terminated and outlive the call.
    unsafe { __android_log_write(INFO, c"Torqa".as_ptr(), text.as_ptr()) };
}
