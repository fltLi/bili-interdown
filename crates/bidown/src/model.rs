//! 互动视频描述数据结构

use std::{fs::File, io::BufReader, path::Path};

use serde::{Deserialize, Serialize};

use crate::impl_pareq_with_id;

/// 互动视频描述
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "video")]
pub struct Video {
    // metadata
    pub id: String,
    pub name: String,
    pub cover: String,
    pub description: String,
    pub author: String,
    // execution
    pub variables: Vec<Variable>,
    pub graph: Graph,
}

impl Video {
    pub fn from_file(path: &Path) -> crate::Result<Self> {
        Ok(serde_json::from_reader(BufReader::new(File::open(path)?))?)
    }
}

/// 变量声明
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "variable")]
pub struct Variable {
    pub id: String,
    pub name: String,
    #[serde(flatten)]
    pub config: VariableConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VariableConfig {
    Normal { default: f64, show: bool },
    Random,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "graph")]
pub struct Graph {
    pub root: usize,
    pub nodes: Vec<Node>,
}

/// 剧情节点
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "node")]
pub struct Node {
    pub id: usize,
    pub name: String,
    #[serde(flatten)]
    pub config: NodeConfig,
}

impl Node {
    pub fn is_leaf(&self) -> bool {
        self.config.is_leaf()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeConfig {
    Choice {
        duration: usize,
        default: Option<usize>,
        choices: Vec<Choice>,
    },
    Leaf,
}

impl NodeConfig {
    pub fn is_leaf(&self) -> bool {
        matches!(self, Self::Leaf)
    }
}

/// 剧情节点选项
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "choice")]
pub struct Choice {
    // metadata
    pub id: usize,
    pub name: String,
    pub target: usize,
    // execution
    pub conditions: Vec<Condition>,
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "condition")]
pub struct Condition {
    #[serde(rename = "type")]
    pub kind: ConditionKind,
    pub id: String,
    pub value: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionKind {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "change")]
pub struct Change {
    #[serde(rename = "type")]
    pub kind: ChangeKind,
    pub id: String,
    pub value: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Set,
    Add,
}

//////// equal ////////

impl_pareq_with_id!(Video);
impl_pareq_with_id!(Variable);
impl_pareq_with_id!(Node);
impl_pareq_with_id!(Choice);
impl_pareq_with_id!(Condition);
impl_pareq_with_id!(Change);
