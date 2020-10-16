pub trait FwSlashEscape {
    fn fw_slash_escape(&self) -> String;
}

pub trait FwSlashUnescape {
    fn fw_slash_unescape(&self) -> String;
}

impl FwSlashEscape for String {
    fn fw_slash_escape(&self) -> String {
        let mut result = self.to_string();
        if self.contains('/') {
            result = self.replace("/", "´´");
        }
        result
    }
}

impl FwSlashUnescape for String {
    fn fw_slash_unescape(&self) -> String {
        let mut result = self.to_string();
        if self.contains("´´") {
            result = self.replace("´´", "/");
        }
        result
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_escape() {
        use crate::utils::escape::*;
        let a = "a/b/c".to_string();
        let b = a.fw_slash_escape();
        assert_eq!(b, "a´´b´´c".to_string());
        let c = b.fw_slash_unescape();
        assert_eq!(a, c);
        let a = "a b c".to_string();
        let b = a.fw_slash_escape();
        assert_eq!(b, "a b c".to_string());
        let c = b.fw_slash_unescape();
        assert_eq!(a, c);
    }
}
