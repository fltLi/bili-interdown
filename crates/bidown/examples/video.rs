//! 互动视频下载示例
//!
//! 此示例将下载 `./demo-{VIDEO}.json` 对应的视频

use std::{env, error::Error, time::Duration};

use bidown::{model::Video, video::Quality};
use env_logger::Env;
use log::{debug, info};
use reqwest::{
    Client,
    header::{
        ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, HeaderMap, HeaderValue, REFERER, USER_AGENT,
    },
};
use reqwest_middleware::ClientBuilder;
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};

const VIDEO: &str = "BV1vSNbzgEQF";
const QUALITY: Quality = Quality::High;

//////// utility ////////

/// 配置请求头
fn headers() -> HeaderMap {
    let mut headers = HeaderMap::new();

    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 Edg/145.0.0.0",
        ),
    );
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/json, text/plain, */*"),
    );
    headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("br, zstd"));
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("zh-CN,zh;q=0.9"));

    // 添加防盗链
    headers.insert(
        REFERER,
        HeaderValue::from_static("https://www.bilibili.com"),
    );

    headers
}

//////// main ////////

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 1. 启动日志
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let root = env::current_dir()?;

    // 2. 解析互动视频描述
    let path = root.join(format!("demo-{VIDEO}.json"));
    debug!("Loading video graph at `{}`", path.to_string_lossy());
    let video = Video::from_file(&path)?;

    // 3. 配置客户端
    debug!("Building client");
    let client = Client::builder().default_headers(headers()).build()?;

    // 4. 配置请求策略
    let retry_policy = ExponentialBackoff::builder()
        .retry_bounds(Duration::from_secs(4), Duration::from_secs(16))
        .build_with_max_retries(3);

    // 5. 构建客户端中间件
    let client = ClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build();

    // 6. 下载相关视频并写入本地文件
    let path = env::current_dir()?.join(format!("demo-{VIDEO}.video"));
    video.download(&client, &path, QUALITY, |_| ()).await?;

    info!("Done! see at `{}`", path.to_string_lossy());
    Ok(())
}
