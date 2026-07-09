//! 模型 id 排序：新版/旗舰优先（可解析版本号降序），供发现、注册表与 profile 写盘共用。

/// 从模型 id 提取数字版本段（如 `glm-5.2` → `[5, 2]`，`kimi-k2.7-code` → `[2, 7]`）。
fn version_tokens(id: &str) -> Vec<u64> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    for ch in id.chars() {
        if ch.is_ascii_digit() {
            cur.push(ch);
        } else if !cur.is_empty() {
            if let Ok(n) = cur.parse::<u64>() {
                tokens.push(n);
            }
            cur.clear();
        }
    }
    if !cur.is_empty() {
        if let Ok(n) = cur.parse::<u64>() {
            tokens.push(n);
        }
    }
    tokens
}

/// 新版优先：`glm-5.2` > `glm-5.1` > `glm-4.7` > `glm-4.5`。
pub(crate) fn compare_models_desc(a: &str, b: &str) -> std::cmp::Ordering {
    let ta = version_tokens(a);
    let tb = version_tokens(b);
    let max_len = ta.len().max(tb.len());
    for i in 0..max_len {
        let va = ta.get(i).copied().unwrap_or(0);
        let vb = tb.get(i).copied().unwrap_or(0);
        match vb.cmp(&va) {
            std::cmp::Ordering::Equal => {}
            other => return other,
        }
    }
    b.cmp(a)
}

pub(crate) fn sort_model_ids(models: &mut [String]) {
    models.sort_by(|a, b| compare_models_desc(a, b));
}

#[cfg(test)]
mod tests {
    use super::{compare_models_desc, sort_model_ids};

    fn sorted(ids: &[&str]) -> Vec<String> {
        let mut v: Vec<String> = ids.iter().map(|s| s.to_string()).collect();
        sort_model_ids(&mut v);
        v
    }

    #[test]
    fn glm_versions_newest_first() {
        assert_eq!(
            sorted(&["glm-4.5", "glm-4.7", "glm-5.2"]),
            vec!["glm-5.2", "glm-4.7", "glm-4.5"]
        );
    }

    #[test]
    fn glm_minor_versions() {
        assert_eq!(sorted(&["glm-5.1", "glm-5.2"]), vec!["glm-5.2", "glm-5.1"]);
    }

    #[test]
    fn kimi_k_series() {
        assert_eq!(
            sorted(&["kimi-k2.6", "kimi-k2.7-code", "kimi-k2.7-code-highspeed"]),
            vec!["kimi-k2.7-code-highspeed", "kimi-k2.7-code", "kimi-k2.6"]
        );
    }

    #[test]
    fn compare_is_consistent_with_sort() {
        assert_eq!(
            compare_models_desc("glm-5.2", "glm-4.5"),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            compare_models_desc("glm-4.5", "glm-5.2"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            compare_models_desc("glm-5.2", "glm-5.2"),
            std::cmp::Ordering::Equal
        );
    }
}
