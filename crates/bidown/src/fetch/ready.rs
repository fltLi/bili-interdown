//! 元数据和准备信息爬取

use log::debug;
use reqwest_middleware::ClientWithMiddleware;
use serde::Deserialize;
use thiserror::Error;

use super::Response;

use crate::model::{Graph, Variable, Video};

//////// metadata ////////

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Metadata {
    #[serde(rename = "bvid")]
    id: String,
    #[serde(rename = "cid")]
    root: usize,
    #[serde(rename = "title")]
    name: String,
    #[serde(rename = "pic")]
    cover: String,
    #[serde(rename = "desc")]
    description: String,
    owner: Owner,
}

impl Metadata {
    pub fn into_video(self, variables: Vec<Variable>, graph: Graph) -> Video {
        let Self {
            id,
            name,
            cover,
            description,
            owner,
            ..
        } = self;
        let Owner { name: author } = owner;

        Video {
            id,
            name,
            cover,
            description,
            author,
            variables,
            graph,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Owner {
    pub name: String,
}

//////// version ////////

#[derive(Debug, Clone, Deserialize)]
struct Version {
    interaction: Interaction,
}

#[derive(Debug, Clone, Deserialize)]
struct Interaction {
    #[serde(rename = "graph_version")]
    version: usize,
}

//////// service ////////

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    ReqwestMiddleware(#[from] reqwest_middleware::Error),

    #[error(transparent)]
    Reqwest(#[from] reqwest_middleware::reqwest::Error),

    #[error("视频分页数要求为 1, 而实际为 {0}")]
    PagesCount(usize),

    #[error("视频不为互动视频或找不到版本信息: {0}")]
    VersionNotFound(reqwest_middleware::reqwest::Error),
}

/// 爬取元数据和根节点 cid
pub async fn fetch_metadata(
    client: &ClientWithMiddleware,
    bvid: &str,
) -> Result<(Metadata, usize)> {
    let url = format!("https://api.bilibili.com/x/web-interface/view?bvid={bvid}");
    debug!("Fetching metadata from `{url}`");
    let response = client.get(url).send().await?;
    let metadata = response.json::<Response<Metadata>>().await?.data;
    let root = metadata.root;
    Ok((metadata, root))
}

/// 爬取互动视频版本信息
pub async fn fetch_version(client: &ClientWithMiddleware, bvid: &str, cid: usize) -> Result<usize> {
    let url = format!("https://api.bilibili.com/x/player/v2?cid={cid}&bvid={bvid}");
    debug!("Fetching graph version from `{url}`");
    let response = client.get(url).send().await?;
    let version: Response<Version> = response.json().await.map_err(Error::VersionNotFound)?;
    Ok(version.data.interaction.version)
}

#[cfg(test)]
mod test {
    use crate::fetch::ready::{Metadata, Owner};

    #[test]
    fn test_metadata_deserialize() {
        assert_eq!(
            serde_json::from_str::<Metadata>(
                r#"{"bvid":"BV..","pic":"https://...jpg","title":"VIDEO_TITLE","desc":"VIDEO_DESCRIPTION","owner":{"name":"VIDEO_AUTHOR"},"cid":1}"#
            ).unwrap(),
            Metadata {
                id: "BV..".to_string(),
                root: 1,
                name: "VIDEO_TITLE".to_string(),
                cover: "https://...jpg".to_string(),
                description: "VIDEO_DESCRIPTION".to_string(),
                owner: Owner {
                    name: "VIDEO_AUTHOR".to_string()
                }
            }
        );
    }
}
