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
    }
}

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
        ];
        let codes: Vec<&str> = all.iter().map(code_of).collect();
        assert_eq!(codes[0], "NotFound");
        assert_eq!(codes[11], "CorruptImage");
        let mut unique = codes.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), all.len());
    }
}
