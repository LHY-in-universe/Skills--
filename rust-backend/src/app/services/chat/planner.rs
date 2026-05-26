use crate::domain::models::RuntimeSettings;

/// planner 启用条件，沿用 Python 版的稳定启发式规则。
pub fn should_plan(runtime: &RuntimeSettings, query: &str, tier: &str) -> bool {
    if !runtime.planner_enabled {
        return false;
    }
    let q = query.trim();
    if tier == "hard" {
        return true;
    }
    q.chars().count() >= 80
        && [
            "并且",
            "同时",
            "步骤",
            "方案",
            "implement",
            "plan",
            "optimize",
        ]
        .iter()
        .any(|k| q.to_lowercase().contains(&k.to_lowercase()))
}

/// 构造计划步骤。
pub fn build_plan_steps(runtime: &RuntimeSettings, query: &str) -> Vec<String> {
    let max_steps = runtime.planner_max_steps.clamp(2, 8) as usize;
    let seeds = vec![
        "澄清目标与约束".to_string(),
        "拆分子问题并确定优先级".to_string(),
        "逐步实现与验证关键路径".to_string(),
        "汇总结果并标注风险".to_string(),
    ];
    let parts = query
        .split(['，', '。', '；', ';', ',', '.'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let dynamic = parts
        .iter()
        .take(max_steps.saturating_sub(seeds.len()))
        .map(|p| format!("处理: {}", p.chars().take(28).collect::<String>()))
        .collect::<Vec<_>>();
    let mut steps = Vec::new();
    steps.extend(seeds.iter().take(2).cloned());
    steps.extend(dynamic);
    steps.extend(seeds.iter().skip(2).cloned());
    steps.truncate(max_steps);
    if steps.is_empty() {
        vec![
            "分析问题".to_string(),
            "执行实现".to_string(),
            "结果校验".to_string(),
        ]
    } else {
        steps
    }
}

/// 轻量质量审计，用于自我纠错重试。
pub fn audit_answer(query: &str, answer: &str) -> (bool, String) {
    let q = query.trim();
    let a = answer.trim();
    if a.is_empty() {
        return (false, "回答为空".to_string());
    }
    if a.chars().count() < 20 {
        return (false, "回答过短，信息不足".to_string());
    }
    if (q.contains('?') || q.contains('？'))
        && (a.contains("不知道") || a.contains("无法"))
        && a.chars().count() < 80
    {
        return (false, "未有效回答问题".to_string());
    }
    if ["步骤", "如何", "怎么"].iter().any(|k| q.contains(k))
        && !["1.", "2.", "步骤", "首先"].iter().any(|k| a.contains(k))
    {
        return (false, "缺少可执行步骤".to_string());
    }
    (true, "passed".to_string())
}
