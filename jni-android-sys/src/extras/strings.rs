#![cfg(any(feature = "all", feature = "java-lang-String"))]

use super::java;

use jni_glue::*;

use std::char::DecodeUtf16Error;
use std::fmt::{self, Debug, Formatter};

impl java::lang::String {
    /// Create new local string from an Env + AsRef<str>
    pub fn from_env_str<'env, S: AsRef<str>>(env: &'env Env, string: S) -> Local<'env, Self> {
        let chars = string.as_ref().encode_utf16().collect::<Vec<_>>();

        let string = unsafe {
            env.new_string(
                chars.as_ptr() as *const jchar,
                chars.len() as jni_sys::jsize,
            )
        };

        unsafe { Local::from_env_object(env.as_jni_env() as *const _, string) }
    }

    fn string_chars(&self) -> StringChars {
        unsafe {
            let env = Env::from_ptr(self.0.env);
            StringChars::from_env_jstring(env, self.0.object)
        }
    }

    /// Returns a new [Ok]\([String]\), or an [Err]\([DecodeUtf16Error]\) if if it contained any invalid UTF16.
    ///
    /// [Ok]:                       https://doc.rust-lang.org/std/result/enum.Result.html#variant.Ok
    /// [Err]:                      https://doc.rust-lang.org/std/result/enum.Result.html#variant.Err
    /// [DecodeUtf16Error]:         https://doc.rust-lang.org/std/char/struct.DecodeUtf16Error.html
    /// [String]:                   https://doc.rust-lang.org/std/string/struct.String.html
    /// [REPLACEMENT_CHARACTER]:    https://doc.rust-lang.org/std/char/constant.REPLACEMENT_CHARACTER.html
    pub fn to_string(&self) -> Result<String, DecodeUtf16Error> {
        self.string_chars().to_string()
    }

    /// Returns a new [String] with any invalid UTF16 characters replaced with [REPLACEMENT_CHARACTER]s (`'\u{FFFD}'`.)
    ///
    /// [String]:                   https://doc.rust-lang.org/std/string/struct.String.html
    /// [REPLACEMENT_CHARACTER]:    https://doc.rust-lang.org/std/char/constant.REPLACEMENT_CHARACTER.html
    pub fn to_string_lossy(&self) -> String {
        self.string_chars().to_string_lossy()
    }
}

// OsString doesn't implement Display, so neither does java::lang::String.

impl Debug for java::lang::String {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.to_string_lossy(), f) // XXX: Unneccessary alloc?  Shouldn't use lossy here?
    }
}
