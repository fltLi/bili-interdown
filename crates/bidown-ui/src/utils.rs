use anyhow::Error;
use rfd::{MessageDialog, MessageLevel};

pub fn show_error(e: &Error) {
    let _ = MessageDialog::new()
        .set_level(MessageLevel::Error)
        .set_title("错误")
        .set_description(e.to_string())
        .show();
}
