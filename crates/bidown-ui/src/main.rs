//! bidown slint ui

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

slint::include_modules!();

use std::{fs, path::PathBuf};

use anyhow::Result;
use log::{LevelFilter, debug};
use rfd::FileDialog;
use slint::ComponentHandle;

use crate::fetch::bind_fetch;

mod fetch;
mod utils;

//////// log ////////

const LOG_DIR: &str = "./log";
const LOG_LEVEL: LevelFilter = LevelFilter::Info;

fn init_log() -> Result<()> {
    let time = chrono::Local::now();
    let date = time.format("%Y-%m-%d").to_string();

    let dir = PathBuf::from(LOG_DIR).join(date);
    fs::create_dir_all(&dir)?;

    let date = time.format("%H-%M-%S").to_string();
    let path = dir.join(format!("{date}.log"));

    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {} {}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(LOG_LEVEL)
        .chain(fern::log_file(path)?)
        .apply()?;

    Ok(())
}

//////// main ////////

fn bind_utils(utils: Utility<'_>) {
    utils.on_pick_folder(|| {
        debug!("Open folder picker");
        let result = FileDialog::new()
            .set_title("选择文件夹")
            .set_can_create_directories(true)
            .pick_folder();
        debug!("User pick folder result: `{result:?}`");
        let path = result.and_then(|p| p.to_str().map(str::to_string));
        path.unwrap_or_default().into()
    });
}

fn open() -> Result<MainWindow> {
    debug!("Initializing UI...");
    let ui = MainWindow::new()?;

    bind_utils(ui.global::<Utility>());
    bind_fetch(ui.global::<Fetch>(), ui.as_weak());

    debug!("UI initialized");
    Ok(ui)
}

fn main() -> Result<()> {
    init_log()?;
    open()?.run()?;
    Ok(())
}
