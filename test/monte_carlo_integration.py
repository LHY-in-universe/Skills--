#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
蒙特卡洛积分计算器
作者: ClawTest
用户: Sam

使用五种不同的蒙特卡洛方法计算一维定积分，并对比精度和效率。
"""

import numpy as np
import time
from math import sin, exp, sqrt
from typing import Callable, Tuple


class MonteCarloIntegrator:
    """蒙特卡洛积分计算器"""
    
    def __init__(self, seed: int = 42):
        np.random.seed(seed)
        self.methods = {
            'crude': '基础随机采样',
            'hit_or_miss': '打靶法',
            'stratified': '分层采样',
            'importance': '重要性采样',
            'control_variates': '控制变量法'
        }
    
    @staticmethod
    def get_function(func_name: str) -> Callable:
        """获取被积函数及其真实值"""
        if func_name == 'sin':
            f = lambda x: sin(np.pi * x)
            true_value = 2 / np.pi
        elif func_name == 'x2':
            f = lambda x: x ** 2
            true_value = 1 / 3
        elif func_name == 'exp':
            f = lambda x: exp(x)
            true_value = exp(1) - 1
        else:
            raise ValueError(f'未知函数: {func_name}')
        return f, true_value
    
    def crude(self, f: Callable, n: int) -> Tuple[float, float]:
        """基础随机采样"""
        x = np.random.random(n)
        y = f(x)
        estimate = np.mean(y)
        variance = np.var(y) / n
        return estimate, variance
    
    def hit_or_miss(self, f: Callable, n: int) -> Tuple[float, float]:
        """打靶法"""
        x = np.random.random(n)
        max_y = 2.0
        y = np.random.random(n) * max_y
        hit = y <= f(x)
        estimate = np.mean(hit) * max_y
        variance = np.var(hit * max_y) / n
        return estimate, variance
    
    def stratified(self, f: Callable, n: int, m: int = 10) -> Tuple[float, float]:
        """分层采样"""
        k = m
        n_per_stratum = n // k
        estimates = []
        variances = []
        
        for i in range(k):
            x = np.random.random(n_per_stratum) / k + i / k
            y = f(x)
            estimates.append(np.mean(y))
            variances.append(np.var(y) / n_per_stratum)
        
        estimate = np.mean(estimates)
        variance = np.sum(variances) / k
        return estimate, variance
    
    def importance(self, f: Callable, n: int) -> Tuple[float, float]:
        """重要性采样"""
        x = np.sqrt(np.random.random(n))
        weights = f(x) / (2 * x + 1e-10)
        estimate = np.mean(weights)
        variance = np.var(weights) / n
        return estimate, variance
    
    def control_variates(self, f: Callable, n: int) -> Tuple[float, float]:
        """控制变量法"""
        x = np.random.random(n)
        f_values = f(x)
        g_values = x
        
        cov_f_g = np.cov(f_values, g_values, ddof=0)[0, 1]
        var_g = np.var(g_values)
        
        if var_g < 1e-10:
            return self.crude(f, n)
        
        c = cov_f_g / var_g
        integral_g = 1 / 2
        estimate = np.mean(f_values - c * (g_values - integral_g))
        variance = np.var(f_values - c * (g_values - integral_g)) / n
        return estimate, variance
    
    def integrate(self, func: str, n: int = 100000, method: str = 'all') -> dict:
        """计算积分"""
        f, true_value = self.get_function(func)
        results = {}
        
        if method == 'all':
            methods = ['crude', 'hit_or_miss', 'stratified', 'importance', 'control_variates']
        else:
            methods = [method]
        
        start_time = time.time()
        
        for m in methods:
            if m == 'stratified':
                estimate, variance = self.stratified(f, n)
            else:
                func_method = getattr(self, m)
                estimate, variance = func_method(f, n)
            
            abs_error = abs(estimate - true_value)
            rel_error = abs_error / abs(true_value)
            std_dev = sqrt(variance)
            
            results[m] = {
                'method_name': self.methods[m],
                'estimate': estimate,
                'true_value': true_value,
                'abs_error': abs_error,
                'rel_error': rel_error,
                'std_dev': std_dev,
                'variance': variance
            }
        
        elapsed = time.time() - start_time
        return {
            'function': func,
            'n': n,
            'results': results,
            'elapsed_time': elapsed
        }
    
    def print_results(self, report: dict):
        """打印结果"""
        print('\n' + '=' * 80)
        print(f"蒙特卡洛积分结果 - 函数: {report['function']}@[0,1], 采样点数: {report['n']}")
        print('=' * 80)
        
        results = report['results']
        sorted_results = sorted(results.values(), key=lambda x: x['abs_error'])
        
        print(f"{'排名':<4} {'方法':<16} {'估计值':<12} {'真实值':<12} {'绝对误差':<12} {'相对误差':<12}")
        print('-' * 80)
        
        for i, r in enumerate(sorted_results, 1):
            print(f"{i:<4} {r['method_name']:<16} "
                  f"{r['estimate']:<12.6f} "
                  f"{r['true_value']:<12.6f} "
                  f"{r['abs_error']:<12.6f} "
                  f"{r['rel_error']*100:<11.4f}%")
        
        print('=' * 80)
        best = sorted_results[0]
        print(f"最佳方法: {best['method_name']} (误差: {best['abs_error']:.6f})")
        print('=' * 80 + '\n')


def main():
    """主函数"""
    integrator = MonteCarloIntegrator(seed=42)
    
    print("""
============================================================
           蒙特卡洛积分计算器
                  作者: ClawTest
============================================================

可用函数:
  sin   - sin(πx) 在 [0,1] 上，真实值 = 2/π
  x2    - x² 在 [0,1] 上，真实值 = 1/3
  exp   - e^x 在 [0,1] 上，真实值 = e¹ - 1

可用方法:
  crude             - 基础随机采样
  hit_or_miss       - 打靶法
  stratified        - 分层采样
  importance        - 重要性采样
  control_variates  - 控制变量法
  all               - 运行所有方法并对比
""")
    
    # 默认参数
    func = 'sin'
    n = 100000
    method = 'all'
    
    # 计算积分
    print(f"计算 {func} 函数的积分，使用 '{method}' 方法，采样点数: {n}")
    report = integrator.integrate(func=func, n=n, method=method)
    
    # 打印结果
    integrator.print_results(report)


if __name__ == '__main__':
    main()