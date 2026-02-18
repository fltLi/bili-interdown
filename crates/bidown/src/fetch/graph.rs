//! 剧情图爬取

use std::{collections::HashSet, str::FromStr};

use log::{debug, info};
use reqwest_middleware::ClientWithMiddleware;
use serde::Deserialize;
use serde_repr::Deserialize_repr;
use serde_with::{BoolFromInt, serde_as};
use thiserror::Error;

use super::Response;

use crate::{
    model::{
        self, Change, ChangeKind, Condition, ConditionKind, Graph, Node, NodeConfig, VariableConfig,
    },
    utils::one_or_len,
};

//////// variable ////////

#[derive(Debug, Clone, Deserialize)]
struct Gate {
    #[serde(rename = "edge_id")]
    id: usize,
    #[serde(rename = "hidden_vars")]
    variables: Vec<Variable>,
}

impl From<Gate> for (Vec<model::Variable>, usize) {
    fn from(value: Gate) -> Self {
        let Gate { id, variables } = value;
        (variables.into_iter().map(Into::into).collect(), id)
    }
}

#[serde_as]
#[derive(Debug, Clone, Deserialize)]
struct Variable {
    #[serde(rename = "id_v2")]
    id: String,
    name: String,
    #[serde(rename = "type")]
    kind: VariableKind,
    #[serde(rename = "value", default)]
    default: f64,
    #[serde_as(as = "BoolFromInt")]
    #[serde(rename = "is_show", default)]
    show: bool,
}

impl From<Variable> for model::Variable {
    fn from(value: Variable) -> Self {
        let Variable {
            id,
            name,
            kind,
            default,
            show,
        } = value;
        let config = match kind {
            VariableKind::Normal => VariableConfig::Normal { default, show },
            VariableKind::Random => VariableConfig::Random,
        };

        Self { id, name, config }
    }
}

#[derive(Debug, Clone, Deserialize_repr)]
#[repr(u8)]
enum VariableKind {
    Normal = 1,
    Random = 2,
}

//////// node ////////

#[serde_as]
#[derive(Debug, Clone, Deserialize)]
struct Edge {
    #[serde(rename = "title")]
    name: String,
    #[serde_as(as = "BoolFromInt")]
    #[serde(rename = "is_leaf", default)]
    leaf: bool,
    #[serde(rename = "edges", default)]
    config: EdgeConfig,
}

impl Edge {
    fn into_node(self, id: usize) -> Result<Node> {
        let Self { name, leaf, config } = self;

        let config = if leaf {
            NodeConfig::Leaf
        } else {
            config.try_into()?
        };

        Ok(Node { id, name, config })
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
struct EdgeConfig {
    #[serde(rename = "questions", default)] // leaf (见下)
    choices: Vec<Choices>,
}

impl TryFrom<EdgeConfig> for NodeConfig {
    type Error = Error;

    fn try_from(value: EdgeConfig) -> Result<Self> {
        // leaf 时走不到这里
        one_or_len(value.choices)
            .map_err(Error::ChoicesCount)?
            .try_into()
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Choices {
    // type = 0
    duration: isize, // 处理 duration = -1 -> 视为瞬间播放
    choices: Vec<Choice>,
}

impl TryFrom<Choices> for NodeConfig {
    type Error = Error;

    fn try_from(value: Choices) -> Result<Self> {
        let Choices { duration, choices } = value;

        let duration = duration.try_into().unwrap_or(0);
        let default = Choice::find_default(&choices).map(|c| c.id);

        let choices = choices
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<model::Choice>>>()?;

        Ok(Self::Choice {
            duration,
            default,
            choices,
        })
    }
}

#[serde_as]
#[derive(Debug, Clone, Deserialize)]
struct Choice {
    id: usize,
    #[serde(rename = "option", default)]
    name: String,
    #[serde(rename = "cid")]
    target: usize,
    #[serde(rename = "condition", default)]
    conditions: String,
    #[serde(rename = "native_action", default)]
    changes: String,
    #[serde_as(as = "BoolFromInt")]
    #[serde(rename = "is_default", default)]
    default: bool,
}

impl Choice {
    fn find_default(choices: &[Choice]) -> Option<&Choice> {
        choices.iter().find(|c| c.default)
    }
}

impl TryFrom<Choice> for model::Choice {
    type Error = Error;

    fn try_from(value: Choice) -> Result<Self> {
        let Choice {
            id,
            name,
            target,
            conditions,
            changes,
            ..
        } = value;

        let conditions =
            Condition::from_str(&conditions).ok_or_else(|| Error::Condition(conditions))?;
        let changes = Change::from_str(&changes).ok_or_else(|| Error::Change(changes))?;

        Ok(Self {
            id,
            name,
            target,
            conditions,
            changes,
        })
    }
}

//////// expression ////////

// ↓ 如果这个行不通, 还有关于条件和修改表达式的另一种处理:
//   直接原样搬到 JS 中执行 (

impl Condition {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(conditions: &str) -> Option<Vec<Self>> {
        macro_rules! split_with_tag {
            ($s:expr, $tag:literal, $kind:expr) => {
                $s.split_once($tag).map(|v| (v, $kind))
            };
            ($s:expr, ($tag0:literal, $kind0:expr), $(($tag:literal, $kind:expr)),* $(,)?) => {{
                split_with_tag!($s, $tag0, $kind0)
                    $(.or_else(|| split_with_tag!($s, $tag, $kind)))*
            }}
        }

        if conditions.trim().is_empty() {
            return Some(Vec::default());
        }

        conditions
            .split("&&")
            .map(|s| {
                let ((id, value), kind) = split_with_tag!(
                    s.trim(),
                    ("<=", ConditionKind::LessEqual),
                    ('<', ConditionKind::Less),
                    (">=", ConditionKind::GreaterEqual),
                    ('>', ConditionKind::Greater),
                    ("!=", ConditionKind::NotEqual),
                    ("==", ConditionKind::Equal),
                )?; // 顺序不能错! 单个符号在多个符号后

                Some(Self {
                    kind,
                    id: id.trim_end().to_string(),
                    value: f64::from_str(value.trim_start()).ok()?,
                })
            })
            .collect()
    }
}

impl Change {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(changes: &str) -> Option<Vec<Self>> {
        macro_rules! strip_with_tag {
            ($s:expr, $tag:literal, $kind:expr) => {
                $s.strip_prefix($tag).map(|v| (v, $kind))
            };
            ($s:expr, ($tag0:literal, $kind0:expr), $(($tag:literal, $kind:expr)),* $(,)?) => {{
                strip_with_tag!($s, $tag0, $kind0)
                    $(.or_else(|| strip_with_tag!($s, $tag, $kind)))*
            }}
        }

        if changes.trim().is_empty() {
            return Some(Vec::default());
        }

        changes
            .split(';')
            .map(|s| {
                let (id, expr) = s.split_once('=')?;
                let (id, expr) = (id.trim(), expr.trim());

                let (value, kind) = expr
                    .strip_prefix(id)
                    .and_then(|s| {
                        // 形如 $id = $id + $value
                        let (value, add_kind) =
                            strip_with_tag!(s.trim_start(), ('+', true), ('-', false))?;

                        let value = {
                            let value = f64::from_str(value).ok()?;
                            if add_kind { value } else { -value }
                        };

                        Some((value, ChangeKind::Add))
                    })
                    .or_else(|| {
                        // 形如 $id = $value
                        let value = f64::from_str(expr).ok()?;
                        Some((value, ChangeKind::Set))
                    })?;

                Some(Self {
                    id: id.to_string(),
                    kind,
                    value,
                })
            })
            .collect()
    }
}

//////// target ////////

struct Target {
    cid: usize,
    eid: usize,
}

impl Node {
    fn list_edges(&self) -> Vec<Target> {
        self.config.list_edges()
    }
}

impl NodeConfig {
    fn list_edges(&self) -> Vec<Target> {
        match self {
            Self::Choice { choices, .. } => choices
                .iter()
                .map(|model::Choice { id, target, .. }| Target {
                    cid: *target,
                    eid: *id,
                })
                .collect(),
            Self::Leaf => Vec::default(),
        }
    }
}

//////// service ////////

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    ReqwestMiddleware(#[from] reqwest_middleware::Error),

    #[error(transparent)]
    Reqwest(#[from] reqwest_middleware::reqwest::Error),

    #[error("节点选项列表数要求为 1, 而实际为 {0}")]
    ChoicesCount(usize),

    #[error("隐藏值条件语句非法: {0}")]
    Condition(String),

    #[error("隐藏值更改语句非法: {0}")]
    Change(String),
}

/// 爬取变量列表
pub async fn fetch_variables(
    client: &ClientWithMiddleware,
    bvid: &str,
    version: usize,
) -> Result<(Vec<model::Variable>, usize)> {
    let url =
        format!("https://api.bilibili.com/x/stein/edgeinfo_v2?bvid={bvid}&graph_version={version}");
    debug!("Fetching variables from {url}");
    let response = client.get(url).send().await?;
    let variables: Response<Gate> = response.json().await?;
    Ok(variables.into())
}

/// 获取并解析节点 (边) 信息
async fn fetch_node(
    client: &ClientWithMiddleware,
    bvid: &str,
    cid: usize,
    eid: usize,
    version: usize,
) -> Result<Node> {
    let url = format!(
        "https://api.bilibili.com/x/stein/edgeinfo_v2?bvid={bvid}&edge_id={eid}&graph_version={version}"
    );
    debug!("Fetching edge info from `{url}`");
    let response = client.get(url).send().await?;
    let edge = response.json::<Response<Edge>>().await?.data;
    edge.into_node(cid)
}

/// 爬取剧情图
pub async fn fetch_graph(
    client: &ClientWithMiddleware,
    bvid: &str,
    root: usize,
    root_eid: usize,
    version: usize,
) -> Result<Graph> {
    let mut visit: HashSet<usize> = HashSet::new();
    let mut nodes = Vec::new(); // 其实可以预先计算容量的说 (

    let mut stack = vec![Target {
        cid: root,
        eid: root_eid,
    }];
    while let Some(Target { cid, eid }) = stack.pop() {
        if !visit.insert(cid) {
            // 标记为已获取
            continue;
        }

        let node = fetch_node(client, bvid, cid, eid, version).await?;
        info!("Node `{}` fetched, name=`{}`", node.id, node.name);

        stack.append(&mut node.list_edges()); // 推入邻边
        nodes.push(node);
    }

    Ok(Graph { root, nodes })
}

//////// test ////////

#[cfg(test)]
mod test {
    use super::{/*Edge, Response,*/ Variable};

    use crate::model::{self, Change, ChangeKind, Condition, ConditionKind, VariableConfig};

    #[test]
    fn test_variable_deserialize() {
        assert_eq!(
            <Variable as Into<model::Variable>>::into(
                serde_json::from_str::<Variable>(
                    r#"{"value":0,"id_v2":"$v1","type":1,"is_show":1,"name":"VAR_1"}"#
                )
                .unwrap()
            ),
            model::Variable {
                id: "$v1".to_string(),
                name: "VAR_1".to_string(),
                config: VariableConfig::Normal {
                    default: 0.,
                    show: true
                }
            }
        );

        assert_eq!(
            <Variable as Into<model::Variable>>::into(
                serde_json::from_str::<Variable>(
                    r#"{"value":0,"id_v2":"$v2","type":2,"is_show":0,"name":"VAR_2"}"#
                )
                .unwrap()
            ),
            model::Variable {
                id: "$v2".to_string(),
                name: "VAR_2".to_string(),
                config: VariableConfig::Random
            }
        );
    }

    // #[test]
    // fn test_edge_deserialize() {
    //     let text = r#"{}"#;
    //     let _response: Response<Edge> = serde_json::from_str(text).unwrap();
    // }

    #[test]
    fn test_conditions_deserialize() {
        assert_eq!(
            // 注意到 PartialEq 被重载了 (
            Condition::from_str("$v1<=1.00 && $v2>2.00"),
            Some(vec![
                Condition {
                    kind: ConditionKind::LessEqual,
                    id: "$v1".to_string(),
                    value: 1.00,
                },
                Condition {
                    kind: ConditionKind::Greater,
                    id: "$v2".to_string(),
                    value: 2.00
                }
            ])
        );
    }

    #[test]
    fn test_changes_deserialize() {
        assert_eq!(
            // 注意到 PartialEq 被重载了 (
            Change::from_str("$v1=1.00;$v2=$v2+0.50"),
            Some(vec![
                Change {
                    kind: ChangeKind::Set,
                    id: "$v1".to_string(),
                    value: 1.00
                },
                Change {
                    kind: ChangeKind::Add,
                    id: "$v2".to_string(),
                    value: 0.5
                }
            ])
        );
    }
}
