pub trait FwSlashEscape {
    fn fw_slash_escape(&self) -> String;
}

pub trait FwSlashUnescape {
    fn fw_slash_unescape(&self) -> String;
}

impl FwSlashEscape for String {
    fn fw_slash_escape(&self) -> String {
        if self.contains('/') {
            self.replace("/", "´´")
        } else {
            self.to_string()
        }

    }
}

impl FwSlashUnescape for String {
    fn fw_slash_unescape(&self) -> String {
        if self.contains("´´") {
            self.replace("´´", "/")
        } else {
            self.to_string()
        }
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
