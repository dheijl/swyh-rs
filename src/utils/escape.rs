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
            result = result.replace("/", "\\/");
        }
        if result.contains("|") {
            result = result.replace("|", "``");
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

#[cfg(test)]
mod tests {
    #[test]
    fn test_escape() {
        use crate::utils::escape::*;
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
}
