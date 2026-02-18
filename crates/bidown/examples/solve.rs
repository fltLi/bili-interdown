//! 互动视频求解示例
//!
//! 此示例将求解 `./demo-BV1vSNbzgEQF.json` 对应的视频描述

use std::{env, error::Error, fs::File, io::Write};

use bidown::model::Video;
use env_logger::Env;
use log::{debug, info};

const VIDEO: &str = "BV1vSNbzgEQF";
const MAX_DEPTH: usize = 44;
const CUT_DEPTH: usize = 44;

fn main() -> Result<(), Box<dyn Error>> {
    // 1. 启动日志
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();
    let root = env::current_dir()?;

    // 2. 解析互动视频描述
    let path = root.join(format!("demo-{VIDEO}.json"));
    debug!("Loading video graph at `{}`", path.to_string_lossy());
    let video = Video::from_file(&path)?;

    // 3. 求解
    let solution = video.solve(MAX_DEPTH, CUT_DEPTH, |c| c.id != 43487188)?;

    // 4. 写入本地文件
    let path = root.join(format!("demo-{VIDEO}.sln.json"));
    debug!("Writing to {}", path.to_string_lossy());
    File::create(&path)?.write_all(serde_json::to_string_pretty(&solution)?.as_bytes())?;

    info!("Done! see at `{}`", path.to_string_lossy());
    Ok(())
}
