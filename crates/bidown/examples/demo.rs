//! 使用示例
//!
//! 此示例将获取互动视频 BV1GxLgzgEyL 的相关数据

use std::{env, error::Error, fs::File, io::Write, time::Duration};

use bidown::fetch::fetch;
use env_logger::Env;
use http::Extensions;
use log::{debug, info};
use reqwest::{
    Client, Request, Response,
    header::{ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, HeaderMap, HeaderValue, USER_AGENT},
};
use reqwest_middleware::{ClientBuilder, Middleware, Next};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use tokio::time::sleep;

const VIDEO: &str = "BV1GxLgzgEyL";

//////// utility ////////

struct DelayMiddleware(Duration);

impl DelayMiddleware {
    fn new(delay: Duration) -> Self {
        Self(delay)
    }
}

#[async_trait::async_trait]
impl Middleware for DelayMiddleware {
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<Response> {
        sleep(self.0).await;
        next.run(req, extensions).await
    }
}

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

    headers
}

//////// main ////////

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 1. 启动日志
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    // 2. 配置客户端
    debug!("Building client");
    let client = Client::builder().default_headers(headers()).build()?;

    // 3. 配置请求策略
    let retry_policy = ExponentialBackoff::builder()
        .retry_bounds(Duration::from_secs(4), Duration::from_secs(16))
        .build_with_max_retries(3);

    // 4. 构建客户端中间件
    let client = ClientBuilder::new(client)
        .with(DelayMiddleware::new(Duration::from_millis(500)))
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build();

    // 5. 执行互动视频爬取
    let video = fetch(&client, VIDEO).await?;
    let video = serde_json::to_string_pretty(&video)?;

    // 6. 写入本地文件
    let path = env::current_dir()?.join(format!("demo-{VIDEO}.json"));
    debug!("Writing to {}", path.to_string_lossy());
    File::create(&path)?.write_all(video.as_bytes())?;

    info!("Done! see at `{}`", path.to_string_lossy());
    Ok(())
}
