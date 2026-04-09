pub(crate) trait FwSlashPipeEscape {
    #[cfg(feature = "gui")]
    fn fw_slash_pipe_escape(self) -> String;
    #[allow(dead_code)]
    fn fw_slash_pipe_unescape(self) -> String;
}

impl FwSlashPipeEscape for &str {
    #[cfg(feature = "gui")]
    fn fw_slash_pipe_escape(self) -> String {
        let mut result = String::with_capacity(self.len() + 8);
        for ch in self.chars() {
            match ch {
                '/' => result.push_str("\\/"),
                '|' => result.push_str("``"),
                c => result.push(c),
            }
        }
        result
    }
    fn fw_slash_pipe_unescape(self) -> String {
        let mut result = String::with_capacity(self.len());
        let mut chars = self.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '\\' if chars.peek() == Some(&'/') => {
                    chars.next();
                    result.push('/');
                }
                '`' if chars.peek() == Some(&'`') => {
                    chars.next();
                    result.push('|');
                }
                c => result.push(c),
            }
        }
        result
    }
}

#[cfg(feature = "cli")]
pub(crate) trait SanitizeArg {
    fn sanitize_bool(self) -> String;
}

#[cfg(feature = "cli")]
impl SanitizeArg for &str {
    fn sanitize_bool(self) -> String {
        match self {
            "T" | "t" | "True" | "TRUE" | "Y" | "Yes" | "y" | "yes" | "YES" | "1" => {
                "true".to_string()
            }
            "F" | "f" | "False" | "FALSE" | "N" | "No" | "n" | "no" | "NO" | "0" => {
                "false".to_string()
            }
            _ => self.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_escape() {
        use crate::utils::traits::*;
        let a = "a/b/c|d".to_string();
        let b = a.as_str().fw_slash_pipe_escape();
        assert_eq!(b, "a\\/b\\/c``d".to_string());
        let c = b.as_str().fw_slash_pipe_unescape();
        assert_eq!(a, c);
        let a = "a b c".to_string();
        let b = a.as_str().fw_slash_pipe_escape();
        assert_eq!(b, "a b c".to_string());
        let c = b.as_str().fw_slash_pipe_unescape();
        assert_eq!(a, c);
    }

    #[test]
    fn test_sanitize_bool() {
        use crate::utils::traits::*;
        use lexopt::ValueExt;
        use std::ffi::OsString;
        let args = vec!["T", "t", "True", "TRUE", "Y", "Yes", "YES", "y", "yes", "1"];
        for arg in args {
            assert_eq!(arg.sanitize_bool(), "true");
        }
        let args = vec!["F", "f", "False", "FALSE", "N", "No", "NO", "n", "no", "0"];
        for arg in args {
            assert_eq!(arg.sanitize_bool(), "false");
        }
        assert_eq!("true".sanitize_bool(), "true");
        assert_eq!("false".sanitize_bool(), "false");
        assert_eq!("garbage".sanitize_bool(), "garbage");
        let arg: OsString = "T".into();
        let b: bool = arg.string().unwrap().sanitize_bool().parse().unwrap();
        assert!(b);
    }
}
