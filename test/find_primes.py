#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
求解 1-10000 的所有质数
使用埃拉托斯特尼筛法（Sieve of Eratosthenes）
"""

import math


def sieve_of_eratosthenes(n):
    """
    使用埃拉托斯特尼筛法找出 1 到 n 之间的所有质数
    
    Args:
        n: 上限值（包含）
    
    Returns:
        质数列表
    """
    if n < 2:
        return []
    
    # 创建布尔数组，初始化为 True，表示都是质数
    is_prime = [True] * (n + 1)
    is_prime[0] = is_prime[1] = False  # 0 和 1 不是质数
    
    # 只需要筛到 sqrt(n)
    for i in range(2, int(math.sqrt(n)) + 1):
        if is_prime[i]:
            # 将 i 的所有倍数标记为非质数
            for j in range(i * i, n + 1, i):
                is_prime[j] = False
    
    # 收集所有质数
    primes = [i for i in range(2, n + 1) if is_prime[i]]
    return primes


def main():
    """主函数"""
    upper_limit = 10000
    print(f"正在计算 1 到 {upper_limit} 的所有质数...")
    
    primes = sieve_of_eratosthenes(upper_limit)
    
    print(f"\n共找到 {len(primes)} 个质数")
    print(f"前 50 个质数: {primes[:50]}")
    print(f"后 50 个质数: {primes[-50:]}")
    print(f"\n最大的质数是: {primes[-1]}")
    print(f"最小的质数是: {primes[0]}")


if __name__ == "__main__":
    main()
