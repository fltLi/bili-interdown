//! 互动视频描述爬取

use std::fmt::Debug;

use log::info;
use reqwest_middleware::ClientWithMiddleware;
use thiserror::Error;

use crate::model::Video;

//////// module ////////

mod graph;
use graph::{fetch_graph, fetch_variables};

mod ready;
use ready::{fetch_metadata, fetch_version};

//////// service ////////

/// 爬取过程的返回类型
pub type Result<T> = std::result::Result<T, Error>;

/// 爬取过程的错误类型
#[derive(Debug, Error)]
pub enum Error {
    #[error("元数据和准备信息爬取失败: {0}")]
    Ready(#[from] ready::Error),

    #[error("剧情树爬取失败: {0}")]
    Graph(#[from] graph::Error),
}

/// 爬取互动视频描述
impl Video {
    pub async fn fetch(client: &ClientWithMiddleware, bvid: &str) -> Result<Self> {
        info!("Start fetching video `{bvid}`");

        // 准备工作
        let (metadata, root) = fetch_metadata(client, bvid).await?;
        let version = fetch_version(client, bvid, root).await?;

        // 构建剧情树
        let (variables, root_eid) = fetch_variables(client, bvid, version).await?;
        let graph = fetch_graph(client, bvid, root, root_eid, version).await?;

        info!(
            "Video `{bvid}` fetching done! {} nodes in total",
            graph.nodes.len()
        );
        Ok(metadata.into_video(variables, graph))
    }
}
