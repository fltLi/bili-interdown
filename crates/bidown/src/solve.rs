//! 互动视频求解

use std::{
    collections::{HashMap, HashSet, VecDeque},
    ops::Deref,
    rc::Rc,
};

use log::{debug, info};
use serde::{Serialize, Serializer, ser::SerializeSeq};
use thiserror::Error;

use crate::{
    model::{
        Change, ChangeKind, Choice, Condition, ConditionKind, Graph, Node, NodeConfig, Variable,
        VariableConfig, Video,
    },
    utils::try_all,
};

//////// model ////////

#[derive(Debug, Clone)]
struct StepNext<'a> {
    next: Rc<Step<'a>>,
    choice: &'a Choice,
}

/// 路径 (倒序)
#[derive(Debug, Clone)]
pub struct Step<'a> {
    next: Option<StepNext<'a>>,
    node: &'a Node,
}

impl<'a> Step<'a> {
    fn new(node: &'a Node) -> Self {
        Self { next: None, node }
    }

    fn new_linked(node: &'a Node, choice: &'a Choice, next: Rc<Step<'a>>) -> Self {
        Self {
            next: Some(StepNext { next, choice }),
            node,
        }
    }

    pub fn node(&self) -> &'a Node {
        self.node
    }

    pub fn choice(&self) -> Option<&'a Choice> {
        self.next.as_ref().map(|n| n.choice)
    }

    pub fn iter(self: &Rc<Self>) -> StepIter<'a> {
        StepIter::new(self.clone())
    }

    #[allow(clippy::should_implement_trait)]
    pub fn into_iter(self: Rc<Self>) -> StepIter<'a> {
        StepIter::new(self)
    }
}

impl<'a> From<&Rc<Step<'a>>> for StepIter<'a> {
    fn from(value: &Rc<Step<'a>>) -> Self {
        value.iter()
    }
}

impl<'a> From<Rc<Step<'a>>> for StepIter<'a> {
    fn from(value: Rc<Step<'a>>) -> Self {
        value.into_iter()
    }
}

impl Serialize for Step<'_> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let tail = Rc::new(self.clone()); // clone 开销很小
        let nodes: Vec<StepData> = tail.into_iter().map(|s| s.as_ref().into()).collect();

        let mut seq = serializer.serialize_seq(Some(nodes.len()))?;
        for node in nodes.iter() {
            seq.serialize_element(&node)?;
        }
        seq.end()
    }
}

#[derive(Debug, Clone, Serialize)]
struct StepData<'a> {
    id: usize,
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    edge: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    choice: Option<&'a str>,
}

impl<'a> From<&Step<'a>> for StepData<'a> {
    fn from(value: &Step<'a>) -> Self {
        let Step { next, node } = value;
        Self {
            id: node.id,
            name: &node.name,
            edge: next.as_ref().map(|n| n.choice.id),
            choice: next.as_ref().map(|n| n.choice.name.as_str()),
        }
    }
}

/// 路径迭代器 (倒序)
#[derive(Debug, Clone)]
pub struct StepIter<'a> {
    step: Option<Rc<Step<'a>>>,
}

impl<'a> StepIter<'a> {
    fn new(step: Rc<Step<'a>>) -> Self {
        Self { step: Some(step) }
    }
}

impl<'a> Iterator for StepIter<'a> {
    type Item = Rc<Step<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.step
            .take()
            .inspect(|step| self.step = step.next.as_ref().map(|n| n.next.clone()))
    }
}

/// 解法路径集合 (路径倒序)
#[derive(Debug, Clone, Serialize)]
pub struct Solution<'a>(pub Vec<Rc<Step<'a>>>);

impl<'a> Deref for Solution<'a> {
    type Target = Vec<Rc<Step<'a>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> Solution<'a> {
    /// 遍历叶子节点 (可能是结局)
    pub fn iter_leaf(&self) -> impl Iterator<Item = &Rc<Step<'a>>> {
        self.iter().filter(|s| s.node.is_leaf())
    }
}

//////// context ////////

/// 变量托管
#[derive(Debug, Clone)]
struct Variables<'a> {
    randoms: Rc<HashSet<&'a str>>,
    normals: HashMap<&'a str, f64>,
}

impl<'a> Variables<'a> {
    fn new<I>(vars: I) -> Result<Self>
    where
        I: Iterator<Item = &'a Variable>,
    {
        debug!("Building variables' execution");

        let mut randoms = HashSet::new();
        let mut normals = HashMap::new();

        for Variable { id, config, .. } in vars {
            let id = id.as_str();
            let existed = match config {
                VariableConfig::Normal { default, .. } => normals.insert(id, *default).is_some(),
                VariableConfig::Random if !normals.contains_key(id) => !randoms.insert(id),
                _ => true,
            };

            if existed {
                return Err(Error::RepeatVariable(id.to_string()));
            }
        }

        Ok(Self {
            randoms: Rc::new(randoms),
            normals,
        })
    }

    /// 检查隐藏值是否符合约束
    ///
    /// # Notes
    ///
    /// - 所有随机值产生的判定都将被视为成功
    fn check(&self, condition: &Condition) -> Result<bool> {
        let Condition { kind, id, value } = condition;
        let id = id.as_str();

        // 随机值直接通过
        if self.randoms.contains(id) {
            return Ok(true);
        }

        // 作为 isize 比较
        let variable = *self
            .normals
            .get(id)
            .ok_or_else(|| Error::VariableNotFound(id.to_string()))?
            as isize;
        let value = *value as isize;

        let relation = match kind {
            ConditionKind::Equal => variable == value,
            ConditionKind::NotEqual => variable != value,
            ConditionKind::Less => variable < value,
            ConditionKind::LessEqual => variable <= value,
            ConditionKind::Greater => variable > value,
            ConditionKind::GreaterEqual => variable >= value,
        };
        Ok(relation)
    }

    fn change(&mut self, change: &Change) -> Result<()> {
        let Change { kind, id, value } = change;
        let id = id.as_str();

        // 随机值不管
        if self.randoms.contains(id) {
            return Ok(());
        }

        // 执行更改
        let variable = self
            .normals
            .get_mut(id)
            .ok_or_else(|| Error::VariableNotFound(id.to_string()))?;
        match kind {
            ChangeKind::Add => *variable += value,
            ChangeKind::Set => *variable = *value,
        }

        Ok(())
    }
}

//////// state ////////

const ZOOM_MAGIC: f64 = 24.99;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct State<'a> {
    node: usize,
    variables: Vec<(&'a str, isize)>,
}

impl<'a> State<'a> {
    fn new(node: usize, variables: &HashMap<&'a str, f64>) -> Self {
        let mut variables: Vec<_> = variables
            .iter()
            .map(|(&k, &v)| (k, (v * ZOOM_MAGIC) as isize))
            .collect();
        variables.sort();
        Self { node, variables }
    }
}

//////// service ////////

/// 求解过程的返回类型
pub type Result<T> = std::result::Result<T, Error>;

/// 求解过程的错误类型
#[derive(Debug, Error)]
pub enum Error {
    #[error("节点 {0} 不存在")]
    NodeNotFound(usize),

    #[error("变量 `{0}` 不存在")]
    VariableNotFound(String),

    #[error("变量 `{0}` 重复声明")]
    RepeatVariable(String),
}

impl Graph {
    pub fn nodes_map(&self) -> HashMap<usize, &Node> {
        self.nodes.iter().map(|n| (n.id, n)).collect()
    }

    pub fn nodes_map_filtered<P>(&self, mut pred: P) -> HashMap<usize, &Node>
    where
        P: FnMut(&Node) -> bool,
    {
        self.nodes
            .iter()
            .filter(|&n| pred(n))
            .map(|n| (n.id, n))
            .collect()
    }
}

impl Video {
    /// 求解互动视频
    ///
    /// BFS 全源最短路 + 模拟 (目前只支持每个节点的最短路径计算)
    ///
    /// # Arguments
    ///
    /// - `maxd` - 最大深度限制
    ///
    /// - `cutd` - 超过此深度, 将不再允许经过抵达过的点
    ///
    /// - `pred` - 筛选允许经过的边 (选项), false 时不经过
    ///
    /// - `variable` - 启用隐藏制判定, false 时假定所有判定为真
    ///
    /// # Notes
    ///
    /// - 所有随机值产生的判定都将被视为成功
    pub fn solve<P>(
        &self,
        maxd: usize,
        cutd: usize,
        mut pred: P,
        variable: bool,
    ) -> Result<Solution<'_>>
    where
        P: FnMut(&Choice) -> bool,
    {
        info!("Start solving graph of video `{}`", self.id);

        let total = self.graph.nodes.len();
        let nodes = self.graph.nodes_map();
        let get_node = |id| match nodes.get(&id) {
            Some(&n) => Ok(n),
            None => Err(Error::NodeNotFound(id)),
        };

        let mut solution = Vec::with_capacity(total);
        let mut visit = HashSet::new();
        let mut visit_states = HashSet::new();
        let mut queue = VecDeque::new();

        let root = get_node(self.graph.root)?;
        let variables = Variables::new(self.variables.iter())?;
        queue.push_back((0, Rc::new(Step::new(root)), variables));

        let mut current_dep = 0; // debug!()

        // BFS
        while let Some((dep, step, variables)) = queue.pop_front() {
            let node = step.node();

            // 不走重复状态 (实测, 这个剪枝出乎意料的猛! 相当于走路变成了乘火箭)
            let state = State::new(node.id, &variables.normals);
            if !visit_states.insert(state) {
                continue;
            }

            if dep > current_dep {
                debug!("Current depth: {dep}");
                current_dep = dep;
            }

            // 找到一条路径
            if visit.insert(node.id) {
                solution.push(step.clone());

                let current = solution.len();
                info!(
                    "Node `{}` solved, name=`{}`, progress={current}/{total}",
                    node.id, node.name
                );

                // 找完提前结束
                if current == total {
                    break;
                }
            }

            if dep >= maxd {
                continue;
            }

            let choices = match &node.config {
                NodeConfig::Choice { choices, .. } => choices,
                _ => continue, // 此后只能推入邻边, 不能放其他逻辑!
            };

            // 走到下一个节点
            for choice in choices {
                let Choice {
                    target,
                    conditions,
                    changes,
                    ..
                } = choice;

                // 过滤掉经过的点
                if dep >= cutd && visit.contains(target) {
                    continue;
                }

                // 过滤掉边
                if !pred(choice) {
                    continue;
                }

                // 判定隐藏值
                if variable && !try_all(conditions.iter().map(|c| variables.check(c)))? {
                    continue;
                }

                // 修改隐藏值
                let mut variables = variables.clone();
                for change in changes {
                    variables.change(change)?;
                }

                // 推入邻边
                let node = get_node(*target)?;
                let step = Rc::new(Step::new_linked(node, choice, step.clone()));
                queue.push_back((dep + 1, step, variables));
            }
        }

        info!(
            "Video `{}` solving done! {} of {total} nodes solved in total",
            self.id,
            solution.len()
        );
        Ok(Solution(solution))
    }
}
