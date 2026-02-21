use std::fmt::Debug;

use serde::{Deserialize, Serialize};

/// 工作进度
///
/// # Notes
///
/// - 以节点作为度量单位
///
/// - 若 `total` 未知, 请使用最大值.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    // progress
    pub current: usize,
    pub total: usize,
    // metadata
    pub id: usize,
    pub name: String,
}

/// 单元素 Vec 解包, 失败时返回长度
pub fn one_or_len<T>(mut values: Vec<T>) -> Result<T, usize> {
    match values.pop() {
        Some(v) if values.is_empty() => Ok(v),
        None => Err(0),
        _ => Err(values.len() + 1),
    }
}

/// 遍历可能出错的迭代器检查是否全部为真
pub fn try_all<I, E>(iter: I) -> Result<bool, E>
where
    I: Iterator<Item = Result<bool, E>>,
{
    for r in iter {
        if !matches!(r, Ok(true)) {
            return r;
        }
    }
    Ok(true)
}

/// 依据 id 字段添加 PartialEq 实现
#[macro_export]
macro_rules! impl_pareq_with_id {
    ($t:ty) => {
        paste::paste! {
            impl PartialEq for $t {
                fn eq(&self, other: &Self) -> bool {
                    self.id == other.id
                }
            }
        }
    };
}

/// 响应头便捷包装
#[derive(Debug, Clone, Deserialize)]
pub struct Response<T>
where
    T: Debug + Clone,
{
    pub data: T,
}

impl<T> Response<T>
where
    T: Debug + Clone,
{
    pub fn into<U>(self) -> U
    where
        T: Into<U>,
    {
        self.data.into()
    }

    // pub fn try_into<U>(self) -> std::result::Result<U, T::Error>
    // where
    //     T: TryInto<U>,
    // {
    //     self.data.try_into()
    // }
}
