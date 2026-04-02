# -*- coding: utf-8 -*-
"""
蒙特卡洛积分技能脚本
支持五种方法：基础随机采样、打靶法、分层采样、重要性采样、控制变量法
用法: python3 monte_carlo.py --method <方法> --func <函数> --n <采样数> --seed <种子>
"""

import sys
import json
import math
import random


# ── 1. 被积函数注册表 ─────────────────────────────────────────────────────────
# 格式: 函数名 -> (可调用函数, 区间下界, 区间上界, 真实积分值)
FUNCTIONS = {
    "sin": (math.sin,        0.0, 1.0, 1.0 - math.cos(1.0)),   # ∫sin(x)dx = 1-cos(1)
    "x2":  (lambda x: x**2, 0.0, 1.0, 1.0 / 3.0),              # ∫x²dx = 1/3
    "exp": (math.exp,        0.0, 1.0, math.e - 1.0),           # ∫eˣdx = e-1
}


# ── 2. 辅助工具 ───────────────────────────────────────────────────────────────

def sample_stats(values):
    """计算一组样本的均值和均值标准差（SE = std/√n）"""
    n = len(values)
    if n == 0:
        return 0.0, 0.0
    mean = sum(values) / n
    if n == 1:
        return mean, 0.0
    variance = sum((v - mean) ** 2 for v in values) / (n - 1)
    return mean, math.sqrt(variance / n)


def build_result(method_name, estimate, true_val, std_err, n, extra=None):
    """统一构建单次方法的输出字典"""
    abs_error = abs(estimate - true_val)
    rel_error = abs_error / abs(true_val) if true_val != 0 else float("inf")
    result = {
        "method":     method_name,
        "estimate":   round(estimate, 8),
        "true_value": round(true_val, 8),
        "abs_error":  round(abs_error, 8),
        "rel_error":  round(rel_error, 6),   # 相对误差（小数形式）
        "std_error":  round(std_err, 8),     # 均值标准差
        "n_samples":  n,
    }
    if extra:
        result.update(extra)
    return result


# ── 3. 五种积分方法 ───────────────────────────────────────────────────────────

def method_crude(func, a, b, true_val, n, rng):
    """
    基础随机采样（Crude Monte Carlo）
    在 [a, b] 均匀采样，用函数均值 × 区间长度估计积分：
        I ≈ (b-a) × mean(f(xᵢ))
    """
    samples = [func(rng.uniform(a, b)) for _ in range(n)]
    mean, se = sample_stats(samples)
    estimate = (b - a) * mean
    return build_result("crude", estimate, true_val, (b - a) * se, n)


def method_hit_or_miss(func, a, b, true_val, n, rng):
    """
    打靶法（Hit-or-Miss）
    在矩形 [a,b]×[0, y_max] 内投点，统计命中（落在曲线下方）比例：
        I ≈ (b-a) × y_max × (命中次数 / N)
    """
    # 用粗采样估算函数最大值，加 10% 余量避免漏判
    probe_vals = [func(a + (b - a) * i / 1000) for i in range(1001)]
    y_max = max(probe_vals) * 1.1

    hits = 0
    for _ in range(n):
        x = rng.uniform(a, b)
        y = rng.uniform(0, y_max)
        if y <= func(x):
            hits += 1

    p = hits / n
    estimate = (b - a) * y_max * p
    # 打靶法方差：(b-a)²×y_max²×p(1-p)/n
    se = (b - a) * y_max * math.sqrt(p * (1 - p) / n) if n > 0 else 0.0
    return build_result(
        "hit_or_miss", estimate, true_val, se, n,
        extra={"hits": hits, "y_max": round(y_max, 6)}
    )


def method_stratified(func, a, b, true_val, n, rng):
    """
    分层采样（Stratified Sampling）
    将 [a, b] 等分为 K 层，每层内均匀采样，消除样本聚集。
    层数 K = max(10, √N) 时理论方差最小。
    """
    k = max(10, int(math.sqrt(n)))
    n_per_stratum = max(1, n // k)
    layer_width = (b - a) / k

    layer_means = []
    for i in range(k):
        lo = a + i * layer_width
        hi = lo + layer_width
        vals = [func(rng.uniform(lo, hi)) for _ in range(n_per_stratum)]
        layer_means.append(sum(vals) / n_per_stratum)

    estimate = sum(layer_width * m for m in layer_means)

    # 用层间方差估计整体标准误
    grand_mean = estimate / (b - a)
    se = math.sqrt(
        sum((m - grand_mean) ** 2 for m in layer_means) / max(1, k - 1)
    ) * layer_width / math.sqrt(k)

    return build_result(
        "stratified", estimate, true_val, se, n,
        extra={"k_strata": k, "n_per_stratum": n_per_stratum}
    )


def method_importance(func, a, b, true_val, n, rng):
    """
    重要性采样（Importance Sampling）
    使用截断指数分布 g(x) = λ·exp(-λx) / (1-exp(-λ(b-a))) 作为提案分布，
    用逆变换法精确采样，权重为 f(x)/g(x)：
        I ≈ (1/N) × Σ [f(xᵢ)/g(xᵢ)]，其中 xᵢ ~ g(x)
    """
    lam = 2.0   # 指数分布参数，形状贴近 sin/exp 等单调上升函数

    # 截断指数分布的归一化常数
    c = 1.0 - math.exp(-lam * (b - a))

    def sample_from_g():
        """逆变换法从截断指数分布采样"""
        u = rng.random()
        return a - math.log(1.0 - u * c) / lam

    def pdf_g(x):
        """截断指数分布的 PDF"""
        return lam * math.exp(-lam * (x - a)) / c

    weights = []
    for _ in range(n):
        x = sample_from_g()
        gx = pdf_g(x)
        weights.append(func(x) / gx if gx > 1e-15 else 0.0)

    mean, se = sample_stats(weights)
    return build_result(
        "importance", mean, true_val, se, n,
        extra={"proposal_dist": f"truncated_exp(lambda={lam}, a={a}, b={b})"}
    )


def method_control_variates(func, a, b, true_val, n, rng):
    """
    控制变量法（Control Variates）
    以 h(x) = x 为控制变量（∫[0,1]x dx = 0.5 已知），
    用最优系数 β = Cov(f,h)/Var(h) 修正估计：
        Î_cv = (b-a) × [f̄ - β × (h̄ - μ_h)]
    """
    def h(x):
        return x

    h_true_mean = (b**2 - a**2) / (2.0 * (b - a))   # h 在 [a,b] 上的均值 = (a+b)/2

    # 采样 f 和 h 的配对值
    xs = [rng.uniform(a, b) for _ in range(n)]
    f_vals = [func(x) for x in xs]
    h_vals = [h(x) for x in xs]

    f_mean = sum(f_vals) / n
    h_mean = sum(h_vals) / n

    # 计算最优 β = Cov(f, h) / Var(h)
    cov_fh = sum((f_vals[i] - f_mean) * (h_vals[i] - h_mean) for i in range(n)) / max(1, n - 1)
    var_h  = sum((h_vals[i] - h_mean) ** 2 for i in range(n)) / max(1, n - 1)
    beta = cov_fh / var_h if var_h > 1e-15 else 0.0

    # 修正后的每个样本值
    cv_vals = [f_vals[i] - beta * (h_vals[i] - h_true_mean) for i in range(n)]
    cv_mean, se = sample_stats(cv_vals)
    estimate = (b - a) * cv_mean

    return build_result(
        "control_variates", estimate, true_val, (b - a) * se, n,
        extra={
            "control_var":    "h(x)=x",
            "beta":           round(beta, 6),
            "h_true_integral": round(h_true_mean * (b - a), 8),
        }
    )


# ── 4. 方法分派表 ─────────────────────────────────────────────────────────────
METHODS = {
    "crude":            method_crude,
    "hit_or_miss":      method_hit_or_miss,
    "stratified":       method_stratified,
    "importance":       method_importance,
    "control_variates": method_control_variates,
}


# ── 5. 参数解析 ───────────────────────────────────────────────────────────────

def parse_args(argv):
    """简单的 --key value 和 --key=value 解析"""
    args = {
        "method": "crude",
        "func":   "sin",
        "n":      "100000",
        "seed":   "42",
    }
    i = 1
    while i < len(argv):
        arg = argv[i]
        if "=" in arg and arg.startswith("--"):
            k, v = arg.lstrip("-").split("=", 1)
            args[k] = v
        elif arg.startswith("--") and i + 1 < len(argv):
            args[arg.lstrip("-")] = argv[i + 1]
            i += 1
        i += 1
    return args


# ── 6. 主逻辑 ─────────────────────────────────────────────────────────────────

def main():
    args = parse_args(sys.argv)

    # 验证被积函数
    func_name = args["func"].lower()
    if func_name not in FUNCTIONS:
        print(json.dumps({
            "error": f"未知函数 '{func_name}'，可选: {list(FUNCTIONS.keys())}"
        }, ensure_ascii=False))
        sys.exit(1)

    func, a, b, true_val = FUNCTIONS[func_name]

    # 验证采样数
    try:
        n = int(args["n"])
        assert n > 0
    except (ValueError, AssertionError):
        print(json.dumps({"error": f"采样数 --n 必须为正整数，当前: {args['n']}"}))
        sys.exit(1)

    # 验证随机种子
    try:
        seed = int(args["seed"])
    except ValueError:
        print(json.dumps({"error": f"随机种子 --seed 必须为整数，当前: {args['seed']}"}))
        sys.exit(1)

    method_name = args["method"].lower()

    # ── 对比模式：all ─────────────────────────────────────────────────────────
    if method_name == "all":
        results = []
        for name, fn in METHODS.items():
            rng_m = random.Random(seed)   # 每种方法独立使用相同种子，保证公平对比
            results.append(fn(func, a, b, true_val, n, rng_m))

        results.sort(key=lambda r: r["abs_error"])   # 按误差从小到大排序

        output = {
            "mode":        "comparison",
            "func":        func_name,
            "integral":    f"integral[{a},{b}] {func_name}(x) dx",
            "true_value":  round(true_val, 8),
            "n_samples":   n,
            "seed":        seed,
            "comparison":  results,
            "best_method": results[0]["method"],
        }
        print(json.dumps(output, indent=2, ensure_ascii=False))
        return

    # ── 单方法模式 ────────────────────────────────────────────────────────────
    if method_name not in METHODS:
        print(json.dumps({
            "error": f"未知方法 '{method_name}'，可选: {list(METHODS.keys())} 或 'all'"
        }, ensure_ascii=False))
        sys.exit(1)

    rng = random.Random(seed)
    result = METHODS[method_name](func, a, b, true_val, n, rng)

    output = {
        "func":     func_name,
        "integral": f"integral[{a},{b}] {func_name}(x) dx",
        "seed":     seed,
        **result,
    }
    print(json.dumps(output, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
