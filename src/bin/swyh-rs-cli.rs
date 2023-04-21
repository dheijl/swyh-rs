use swyh_rs::utils::ui_logger::{disable_ui_log, ui_log};

fn main() {
    disable_ui_log();
    ui_log("Hello World".to_string());
}
