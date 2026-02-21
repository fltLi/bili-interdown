//! 下载页

use std::{
    fs::{self, File},
    io::BufWriter,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

use anyhow::Result;
use bidown::{Progress as ProgressRaw, model::Video, video::Quality};
use log::debug;
use reqwest::{
    Client,
    header::{
        ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, HeaderMap, HeaderValue, REFERER, USER_AGENT,
    },
};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use slint::{ComponentHandle, Weak};
use tokio::runtime::Runtime;

use crate::{Fetch, MainWindow, utils::show_error};

//////// fetch ////////

const MAX_RETRY: u32 = 3;
const MIN_RETRY_INTERVAL: Duration = Duration::from_secs(4);
const MAX_RETRY_INTERVAL: Duration = Duration::from_secs(16);

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

/// 配置客户端
fn client(headers: HeaderMap) -> Result<ClientWithMiddleware> {
    debug!("Building client");

    let client = Client::builder().default_headers(headers).build()?;

    let retry_policy = ExponentialBackoff::builder()
        .retry_bounds(MIN_RETRY_INTERVAL, MAX_RETRY_INTERVAL)
        .build_with_max_retries(MAX_RETRY);
    let retry_middleware = RetryTransientMiddleware::new_with_policy(retry_policy);

    let client = ClientBuilder::new(client).with(retry_middleware).build();
    Ok(client)
}

struct Progress {
    progress: f32,
    message: String,
}

impl Progress {
    fn new(progress: f32, message: impl Into<String>) -> Self {
        Self {
            progress,
            message: message.into(),
        }
    }

    fn with_base(progress: f32, message: impl Into<String>, base: f32, max: f32) -> Self {
        Self::new(base + (max - base) * progress, message)
    }
}

async fn fetch_inner<P>(bvid: &str, path: &Path, quality: Quality, mut progress: P) -> Result<()>
where
    P: FnMut(Progress),
{
    progress(Progress::new(
        0.,
        format!("开始下载视频 `{bvid}`, 质量 `{quality:?}`"),
    ));

    progress(Progress::new(0., "启动客户端..."));
    let client = client(headers())?;

    progress(Progress::new(0.05, "爬取剧情树..."));
    let video = Video::fetch(
        &client,
        bvid,
        |ProgressRaw {
             current, id, name, ..
         }| {
            progress(Progress::new(
                0.05,
                format!("[节点收集: {current}] 发现节点 {id}: `{name}`"),
            ))
        },
    )
    .await?;

    progress(Progress::new(0.2, "保存视频信息到本地..."));
    fs::create_dir_all(path)?;
    let data_path = path.join("data.json");
    serde_json::to_writer_pretty(BufWriter::new(File::create(data_path)?), &video)?;

    progress(Progress::new(0.25, "下载节点视频..."));
    let video_path = path.join("video");
    fs::create_dir(&video_path)?;
    video
        .download(
            &client,
            &video_path,
            quality,
            |ProgressRaw {
                 current,
                 total,
                 id,
                 name,
             }| {
                progress(Progress::with_base(
                    current as f32 / total as f32,
                    format!("[视频下载: {current}/{total}] 下载节点视频 {id}: `{name}`"),
                    0.2,
                    1.,
                ))
            },
        )
        .await?;

    progress(Progress::new(
        0.,
        format!("完成下载! 位置: `{}`", path.to_string_lossy()),
    ));
    Ok(())
}

//////// bind ////////

const QUALITY: Quality = Quality::High;

#[derive(Debug, Default)]
struct ProgressData {
    progress: f32,
    messages: Vec<String>,
}

impl ProgressData {
    // fn reset(&mut self) {
    //     *self = Self::default();
    // }

    fn update(&mut self, progress: Progress) {
        let Progress { progress, message } = progress;
        self.progress = progress;
        self.messages.push(message);
    }

    fn messages(&self) -> String {
        self.messages.join("\n")
    }
}

pub fn bind_fetch<'a>(fetch: Fetch<'a>, ui: Weak<MainWindow>) {
    fetch.on_fetch(move |bvid, path| {
        ui.upgrade_in_event_loop(|ui| ui.global::<Fetch>().set_is_fetching(true))
            .unwrap();

        let bvid = bvid.to_string();
        let path = PathBuf::from(path.as_str()).join(&bvid);

        let progress = {
            let ui = ui.clone();
            let progress: Arc<RwLock<ProgressData>> = Arc::default();
            move |p| {
                progress.write().unwrap().update(p);
                let progress = progress.clone();
                ui.upgrade_in_event_loop(move |ui| {
                    debug!("Updating fetching progress...");
                    let progress = progress.read().unwrap();
                    let fetch = ui.global::<Fetch>();
                    fetch.set_progress(progress.progress);
                    fetch.set_log(progress.messages().into());
                })
                .unwrap();
            }
        };

        let _ = thread::spawn({
            let ui = ui.clone();
            move || {
                debug!("Fetching thread spawned");
                let task = fetch_inner(&bvid, &path, QUALITY, progress);

                let runtime = Runtime::new().expect("Failed to create tokio runtime");
                let result = runtime.block_on(task);

                let _ = result.inspect_err(show_error);
                ui.upgrade_in_event_loop(|ui| ui.global::<Fetch>().set_is_fetching(false))
                    .unwrap();
            }
        });
    });
}
