pub trait FwSlashPipeEscape {
    fn fw_slash_pipe_escape(&self) -> String;
}

pub trait FwSlashPipeUnescape {
    fn fw_slash_pipe_unescape(&self) -> String;
}

impl FwSlashPipeEscape for String {
    fn fw_slash_pipe_escape(&self) -> String {
        let mut result: String = self.to_string();
        if result.contains('/') {
            result = result.replace('/', "\\/");
        }
        if result.contains('|') {
            result = result.replace('|', "``");
        }
        result
    }
}

impl FwSlashPipeUnescape for String {
    fn fw_slash_pipe_unescape(&self) -> String {
        let mut result: String = self.to_string();
        if result.contains("\\/") {
            result = result.replace("\\/", "/");
        }
        if result.contains("``") {
            result = result.replace("``", "|");
        }
        result
    }
}

pub trait SanitizeArg {
    fn sanitize_bool(self) -> String;
}

impl SanitizeArg for &str {
    fn sanitize_bool(self) -> String {
        match self {
            "T" | "t" | "True" | "Y" | "Yes" | "y" | "yes" | "1" => "true".to_string(),
            "F" | "f" | "False" | "N" | "No" | "n" | "no" | "0" => "false".to_string(),
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
        let b = a.fw_slash_pipe_escape();
        assert_eq!(b, "a\\/b\\/c``d".to_string());
        let c = b.fw_slash_pipe_unescape();
        assert_eq!(a, c);
        let a = "a b c".to_string();
        let b = a.fw_slash_pipe_escape();
        assert_eq!(b, "a b c".to_string());
        let c = b.fw_slash_pipe_unescape();
        assert_eq!(a, c);
    }

    #[test]
    fn test_sanitize_bool() {
        use crate::utils::traits::*;
        use lexopt::ValueExt;
        use std::ffi::OsString;
        let args = vec!["T", "t", "True", "Y", "Yes", "y", "yes", "1"];
        for arg in args {
            assert_eq!(arg.sanitize_bool(), "true");
        }
        let args = vec!["F", "f", "False", "N", "No", "n", "no", "0"];
        for arg in args {
            assert_eq!(arg.sanitize_bool(), "false");
        }
        assert_eq!("true".sanitize_bool(), "true");
        assert_eq!("false".sanitize_bool(), "false");
        assert_eq!("garbage".sanitize_bool(), "garbage");
        let arg: OsString = "T".into();
        let b: bool = arg.string().unwrap().sanitize_bool().parse().unwrap();
        assert_eq!(b, true);
    }
}
