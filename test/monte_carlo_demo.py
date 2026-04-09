# -*- coding: utf-8 -*-
"""
蒙特卡洛方法计算定积分示例
计算 integral[0,1] sin(x) dx 的值
"""

import random
import math

def monte_carlo_crude(func, a, b, n, seed=42):
    """基础随机采样法"""
    random.seed(seed)
    total = 0
    for _ in range(n):
        x = random.uniform(a, b)
        total += func(x)
    return (b - a) * total / n

def monte_carlo_hit_or_miss(func, a, b, n, seed=42, max_y=None):
    """打靶法（命中或未命中法）"""
    random.seed(seed)
    if max_y is None:
        max_y = 1.0
    
    hits = 0
    for _ in range(n):
        x = random.uniform(a, b)
        y = random.uniform(0, max_y)
        if y <= func(x):
            hits += 1
    
    return (b - a) * max_y * hits / n

def monte_carlo_stratified(func, a, b, n, seed=42):
    """分层采样法"""
    random.seed(seed)
    num_strata = int(math.sqrt(n))
    samples_per_stratum = n // num_strata
    stratum_width = (b - a) / num_strata
    
    total = 0
    for i in range(num_strata):
        stratum_a = a + i * stratum_width
        stratum_b = stratum_a + stratum_width
        stratum_sum = 0
        for _ in range(samples_per_stratum):
            x = random.uniform(stratum_a, stratum_b)
            stratum_sum += func(x)
        total += (stratum_b - stratum_a) * stratum_sum / samples_per_stratum
    
    return total

def monte_carlo_importance(func, a, b, n, seed=42):
    """重要性采样法"""
    random.seed(seed)
    total = 0
    for _ in range(n):
        u = random.random()
        x = math.sqrt(u)
        
        if x > 0:
            total += func(x) / (2 * x)
        else:
            total += func(0.0001) / (2 * 0.0001)
    
    return total / n

def monte_carlo_control_variates(func, a, b, n, seed=42):
    """控制变量法"""
    random.seed(seed)
    known_integral = 0.5
    
    total_f = 0
    total_ctrl = 0
    
    for _ in range(n):
        x = random.uniform(a, b)
        total_f += func(x)
        total_ctrl += x
    
    mean_f = total_f / n
    mean_ctrl = total_ctrl / n
    
    adjusted_estimate = (b - a) * mean_f - (mean_ctrl - known_integral)
    
    return adjusted_estimate

def main():
    def f(x):
        return math.sin(x)
    
    a, b = 0, 1
    true_value = 1 - math.cos(1)
    n_samples = 100000
    
    print("=" * 70)
    print("蒙特卡洛方法计算积分：integral[0,1] sin(x) dx")
    print("真实值:", round(true_value, 6))
    print("=" * 70)
    
    methods = [
        ("基础随机采样法", monte_carlo_crude),
        ("打靶法", monte_carlo_hit_or_miss),
        ("分层采样法", monte_carlo_stratified),
        ("重要性采样法", monte_carlo_importance),
        ("控制变量法", monte_carlo_control_variates)
    ]
    
    results = []
    
    for method_name, method_func in methods:
        try:
            estimate = method_func(f, a, b, n_samples, seed=42)
            abs_error = abs(estimate - true_value)
            rel_error = abs_error / true_value * 100
            
            results.append((method_name, estimate, abs_error, rel_error))
            
            print("")
            print(method_name + ":")
            print("  估计值:", round(estimate, 6))
            print("  绝对误差:", round(abs_error, 6))
            print("  相对误差:", round(rel_error, 4), "%")
        except Exception as e:
            print("")
            print(method_name + ": 执行出错 -", str(e))
    
    print("")
    print("=" * 70)
    print("按绝对误差从小到大排序：")
    print("=" * 70)
    
    results.sort(key=lambda x: x[2])
    for i, (method_name, estimate, abs_error, rel_error) in enumerate(results, 1):
        print(str(i) + ". " + method_name + ":")
        print("   估计值:", round(estimate, 6), ", 绝对误差:", round(abs_error, 6), ", 相对误差:", round(rel_error, 4), "%")

if __name__ == "__main__":
    main()