//! Permessi di sistema su macOS: Accessibilità (mouse e tastiera), Registrazione schermo,
//! Accesso completo al disco (notifiche di sistema). Su Windows non servono.

use serde::Serialize;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Stato {
    pub accessibility: bool,
    pub screen: bool,
    pub full_disk: bool,
    pub needed: bool,
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::c_void;
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        pub fn AXIsProcessTrusted() -> bool;
        pub fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
        pub static kAXTrustedCheckOptionPrompt: *const c_void;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        pub static kCFBooleanTrue: *const c_void;
        pub static kCFTypeDictionaryKeyCallBacks: c_void;
        pub static kCFTypeDictionaryValueCallBacks: c_void;
        pub fn CFDictionaryCreate(alloc: *const c_void, keys: *const *const c_void, values: *const *const c_void, n: isize, kcb: *const c_void, vcb: *const c_void) -> *const c_void;
        pub fn CFRelease(cf: *const c_void);
    }

    /// Chiede l'Accessibilità: macOS mostra la sua richiesta e aggiunge Oltremare all'elenco.
    pub fn chiedi_accessibilita() {
        unsafe {
            let k = [kAXTrustedCheckOptionPrompt];
            let v = [kCFBooleanTrue];
            let d = CFDictionaryCreate(std::ptr::null(), k.as_ptr(), v.as_ptr(), 1, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
            if !d.is_null() {
                AXIsProcessTrustedWithOptions(d);
                CFRelease(d);
            }
        }
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        pub fn CGPreflightScreenCaptureAccess() -> bool;
        pub fn CGRequestScreenCaptureAccess() -> bool;
    }
}

/// Il database delle notifiche è leggibile solo con l'Accesso completo al disco.
pub fn accesso_disco() -> bool {
    #[cfg(target_os = "macos")]
    {
        crate::shell_env::home()
            .map(|h| h.join("Library/Group Containers/group.com.apple.usernoted/db2/db"))
            .is_some_and(|p| std::fs::File::open(p).is_ok())
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

pub fn stato() -> Stato {
    #[cfg(target_os = "macos")]
    unsafe {
        Stato { accessibility: mac::AXIsProcessTrusted(), screen: mac::CGPreflightScreenCaptureAccess(), full_disk: accesso_disco(), needed: true }
    }
    #[cfg(not(target_os = "macos"))]
    {
        Stato { accessibility: true, screen: true, full_disk: true, needed: false }
    }
}

/// Chiede il permesso (dove il sistema lo consente) e apre il pannello delle Impostazioni.
pub fn richiedi(tipo: &str) -> Option<&'static str> {
    #[cfg(target_os = "macos")]
    {
        if tipo == "screen" {
            unsafe {
                mac::CGRequestScreenCaptureAccess();
            }
        }
        if tipo == "accessibility" {
            mac::chiedi_accessibilita();
        }
        if tipo == "fullDisk" {
            // Un tentativo di lettura fa comparire Oltremare nell'elenco dell'Accesso completo al disco
            let _ = accesso_disco();
        }
        Some(match tipo {
            "screen" => "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
            "fullDisk" => "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles",
            "microphone" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
            _ => "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = tipo;
        None
    }
}
