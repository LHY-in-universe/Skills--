# -*- coding: utf-8 -*-
"""
蒙特卡洛积分计算 x^2 在 [0, 1] 区间的积分
使用多种蒙特卡洛方法进行对比
真实值：1/3 ≈ 0.3333
"""
import numpy as np

def f(x):
    return x ** 2

TRUE_VALUE = 1/3

def crude_monte_carlo(n=100000, seed=42):
    np.random.seed(seed)
    x = np.random.uniform(0, 1, n)
    samples = f(x)
    estimate = np.mean(samples)
    std = np.std(samples) / np.sqrt(n)
    return estimate, std

def hit_or_miss(n=100000, seed=42):
    np.random.seed(seed)
    x = np.random.uniform(0, 1, n)
    y = np.random.uniform(0, 1, n)
    hits = np.sum(y <= f(x))
    estimate = hits / n
    p = estimate
    std = np.sqrt(p * (1 - p) / n)
    return estimate, std

def stratified_sampling(n=100000, seed=42, num_strata=10):
    np.random.seed(seed)
    n_per_stratum = n // num_strata
    estimates = []
    for i in range(num_strata):
        a = i / num_strata
        b = (i + 1) / num_strata
        x = np.random.uniform(a, b, n_per_stratum)
        layer_estimate = np.mean(f(x)) * (b - a)
        estimates.append(layer_estimate)
    estimate = sum(estimates)
    variances = []
    for i in range(num_strata):
        a = i / num_strata
        b = (i + 1) / num_strata
        x = np.random.uniform(a, b, n_per_stratum)
        var = np.var(f(x)) / n_per_stratum * ((b - a) ** 2)
        variances.append(var)
    std = np.sqrt(sum(variances))
    return estimate, std

def importance_sampling(n=100000, seed=42):
    np.random.seed(seed)
    x = np.random.beta(2, 1, n)
    weights = f(x) / (2 * x)
    estimate = np.mean(weights)
    std = np.std(weights) / np.sqrt(n)
    return estimate, std

def control_variates(n=100000, seed=42):
    np.random.seed(seed)
    x = np.random.uniform(0, 1, n)
    f_x = f(x)
    g_x = x
    mu_g = 0.5
    cov_fg = np.cov(f_x, g_x)[0, 1]
    var_g = np.var(g_x)
    c = cov_fg / var_g if var_g > 0 else 0
    adjusted = f_x - c * (g_x - mu_g)
    estimate = np.mean(adjusted)
    std = np.std(adjusted) / np.sqrt(n)
    return estimate, std

def run_all_methods(n=100000, seed=42):
    print("=" * 60)
    print("蒙特卡洛积分计算：integral_0^1 x^2 dx")
    print("真实值：{:.6f}".format(TRUE_VALUE))
    print("采样点数：{:,}".format(n))
    print("随机种子：{}".format(seed))
    print("=" * 60)
    
    methods = {
        "基础随机采样 (Crude)": crude_monte_carlo,
        "打靶法 (Hit-or-Miss)": hit_or_miss,
        "分层采样 (Stratified)": stratified_sampling,
        "重要性采样 (Importance)": importance_sampling,
        "控制变量法 (Control Variates)": control_variates,
    }
    
    results = []
    
    for name, method in methods.items():
        estimate, std = method(n, seed)
        abs_error = abs(estimate - TRUE_VALUE)
        rel_error = abs_error / TRUE_VALUE * 100
        results.append({
            "method": name,
            "estimate": estimate,
            "std": std,
            "abs_error": abs_error,
            "rel_error": rel_error
        })
        print("\n{}:".format(name))
        print("  估计值：{:.6f}".format(estimate))
        print("  标准差：{:.6f}".format(std))
        print("  绝对误差：{:.6f}".format(abs_error))
        print("  相对误差：{:.2f}%".format(rel_error))
    
    print("\n" + "=" * 60)
    print("方法对比（按误差从小到大排序）:")
    print("=" * 60)
    results.sort(key=lambda x: x["abs_error"])
    print("{:<25} {:>10} {:>12} {:>10}".format("方法", "估计值", "绝对误差", "相对误差"))
    print("-" * 60)
    for r in results:
        print("{:<25} {:>10.6f} {:>12.6f} {:>10.2f}%".format(
            r['method'], r['estimate'], r['abs_error'], r['rel_error']))
    
    print("=" * 60)

if __name__ == "__main__":
    run_all_methods(n=100000, seed=42)