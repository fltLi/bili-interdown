//! 视频下载

use std::{
    fmt::{Display, Formatter},
    fs,
    path::Path,
};

use bytes::Bytes;
use log::{debug, info};
use reqwest_middleware::ClientWithMiddleware;
use serde::Deserialize;
use thiserror::Error;

use crate::{
    model::{Node, Video},
    utils::Response,
};

//////// download ////////

/// 清晰度枚举
///
/// | 枚举项 | 清晰度 | URL 参数 |
/// | --- | --- | --- |
/// | High | 1080P | 80 |
/// | Medium | 720P | 64 |
/// | Low | 480P | 32 |
/// | VeryLow | 360P | 16 |
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Quality {
    #[default]
    High = 80,
    Medium = 64,
    Low = 32,
    VeryLow = 16,
}

impl Display for Quality {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", *self as u8)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct VideoData {
    durl: Vec<Durl>,
}

impl VideoData {
    fn url(&self) -> Option<&str> {
        self.durl
            .iter()
            .find_map(|Durl { url }| (!url.is_empty()).then_some(url))
            .map(String::as_str)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Durl {
    url: String,
}

async fn download_to_bytes(client: &ClientWithMiddleware, url: &str) -> Result<Bytes> {
    let response = client.get(url).send().await?;
    let bytes = response.bytes().await?;
    Ok(bytes)
}

/// 获取普通 MP4 视频流
async fn fetch_video(
    client: &ClientWithMiddleware,
    bvid: &str,
    cid: usize,
    quality: Quality,
) -> Result<Bytes> {
    let url = format!(
        "https://api.bilibili.com/x/player/playurl?bvid={bvid}&cid={cid}&qn={quality}&fnval=0&otype=json"
    );
    debug!("Fetching video url from `{url}`");
    let response = client.get(url.as_str()).send().await?;
    let data = response.json::<Response<VideoData>>().await?.data;
    let url = data.url().ok_or(Error::StreamNotFound(url))?;
    download_to_bytes(client, url).await
}

/// 下载一个节点的视频
pub async fn download(
    client: &ClientWithMiddleware,
    path: &Path,
    bvid: &str,
    cid: usize,
    quality: Quality,
) -> Result<()> {
    info!(
        "Downloading video node {cid} to `{}`",
        path.to_string_lossy()
    );
    let video = fetch_video(client, bvid, cid, quality).await?;
    fs::write(path, video)?;
    Ok(())
}

//////// service ////////

/// 视频下载过程的返回类型
pub type Result<T> = std::result::Result<T, Error>;

/// 视频下载过程的错误类型
#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error(transparent)]
    ReqwestMiddleWare(#[from] reqwest_middleware::Error),

    #[error("找不到视频流 URL: `{0}`")]
    StreamNotFound(String), // 携带请求 URL

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Video {
    /// 下载关联的视频
    pub async fn download(
        &self,
        client: &ClientWithMiddleware,
        path: &Path,
        quality: Quality,
    ) -> Result<()> {
        let bvid = &self.id;
        fs::create_dir_all(path)?;
        info!("Start downloading video `{bvid}`");

        // 循环下载每一个节点的视频, 按照编号存储在 path/ 下
        for Node { id, .. } in &self.graph.nodes {
            let path = path.join(format!("{id}.mp4"));
            download(client, &path, bvid, *id, quality).await?;
        }

        info!(
            "Video `{bvid}` fetching done! See at `{}`",
            path.to_string_lossy()
        );
        Ok(())
    }
}
