//! Core errors become JS `Error`s carrying a `code` property so a UI can
//! switch on the failure kind without parsing messages.

use wasm_bindgen::JsValue;

pub fn code_of(err: &fs_core::Error) -> &'static str {
    use fs_core::Error::*;
    match err {
        NotFound => "NotFound",
        AlreadyExists => "AlreadyExists",
        InvalidPath => "InvalidPath",
        InvalidName => "InvalidName",
        DiskFull => "DiskFull",
        DirectoryFull => "DirectoryFull",
        NotADirectory => "NotADirectory",
        IsADirectory => "IsADirectory",
        DirectoryNotEmpty => "DirectoryNotEmpty",
        FileTooLarge => "FileTooLarge",
        InvalidGeometry(_) => "InvalidGeometry",
        CorruptImage(_) => "CorruptImage",
        Unsupported(_) => "Unsupported",
        OutOfBounds { .. } => "OutOfBounds",
        NeedsRecovery => "NeedsRecovery",
    }
}

/// Codes the wrapper raises itself, beside the one-per-variant core codes of
/// `code_of`: a malformed argument from JS, and a family-specific method
/// called on a volume of another family.
pub const BAD_ARGUMENT: &str = "BadArgument";
pub const NOT_FAT: &str = "NotFat";
pub const NOT_EXT: &str = "NotExt";

/// A JS `Error` whose `message` is `message` and whose `code` property is `code`.
pub fn js_error(code: &str, message: &str) -> JsValue {
    let e = js_sys::Error::new(message);
    let _ = js_sys::Reflect::set(&e, &JsValue::from_str("code"), &JsValue::from_str(code));
    e.into()
}

pub fn to_js(err: fs_core::Error) -> JsValue {
    js_error(code_of(&err), &err.to_string())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use fs_core::Error;

    #[test]
    fn every_variant_has_its_own_code() {
        let all = [
            Error::NotFound,
            Error::AlreadyExists,
            Error::InvalidPath,
            Error::InvalidName,
            Error::DiskFull,
            Error::DirectoryFull,
            Error::NotADirectory,
            Error::IsADirectory,
            Error::DirectoryNotEmpty,
            Error::FileTooLarge,
            Error::InvalidGeometry("x".into()),
            Error::CorruptImage("x".into()),
            Error::Unsupported("x".into()),
            Error::OutOfBounds {
                offset: 1,
                len: 2,
                disk_len: 3,
            },
            Error::NeedsRecovery,
        ];
        let codes: Vec<&str> = all.iter().map(code_of).collect();
        assert_eq!(codes[0], "NotFound");
        assert_eq!(codes[11], "CorruptImage");
        assert_eq!(codes[13], "OutOfBounds");
        assert_eq!(codes[14], "NeedsRecovery");
        let mut unique = codes.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), all.len());
    }

    #[test]
    fn wrapper_codes_are_distinct_from_every_core_code() {
        let wrapper = [BAD_ARGUMENT, NOT_FAT, NOT_EXT];
        assert_eq!(wrapper, ["BadArgument", "NotFat", "NotExt"]);
        let core = [
            Error::NotFound,
            Error::AlreadyExists,
            Error::InvalidPath,
            Error::InvalidName,
            Error::DiskFull,
            Error::DirectoryFull,
            Error::NotADirectory,
            Error::IsADirectory,
            Error::DirectoryNotEmpty,
            Error::FileTooLarge,
            Error::InvalidGeometry("x".into()),
            Error::CorruptImage("x".into()),
            Error::Unsupported("x".into()),
            Error::OutOfBounds {
                offset: 1,
                len: 2,
                disk_len: 3,
            },
            Error::NeedsRecovery,
        ];
        let core_codes: Vec<&str> = core.iter().map(code_of).collect();
        for code in wrapper {
            assert!(
                !core_codes.contains(&code),
                "{code} collides with a core code"
            );
        }
        let mut all: Vec<&str> = core_codes.iter().copied().chain(wrapper).collect();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), core_codes.len() + wrapper.len());
    }
}
