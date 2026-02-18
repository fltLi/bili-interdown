/// 单元素 Vec 解包, 失败时返回长度
pub fn one_or_len<T>(mut values: Vec<T>) -> Result<T, usize> {
    match values.pop() {
        Some(v) if values.is_empty() => Ok(v),
        None => Err(0),
        _ => Err(values.len() + 1),
    }
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
