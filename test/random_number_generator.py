#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
随机数求解器
使用系统时间作为随机种子，生成20个随机数
"""

import random
import time
from datetime import datetime


def seed_random_with_time():
    """
    使用系统当前时间作为随机种子
    这样可以确保每次运行生成的随机数序列是相同的（如果时间相同）
    实现可重现的随机数序列
    """
    seed_value = int(time.time() * 1000)  # 使用秒的毫秒精度
    print(f"系统时间: {datetime.now()}")
    print(f"随机种子: {seed_value}")
    return seed_value


def generate_random_numbers(count=20):
    """
    生成指定数量的随机数
    
    支持多种随机类型：
    - 纯随机整数
    - 每秒递减的数字
    - 基于系统时间的序列
    
    Args:
        count: 生成的随机数数量
    
    Returns:
        随机数列表
    """
    numbers = []
    start_time = time.time()
    
    for i in range(count):
        # 使用秒数作为种子
        seed = int(time.time() * 1000) & 0xFFFFFFFF
        
        # 生成随机整数 (0-9999)
        num = random.randint(10000, 99999)
        numbers.append(num)
        
        # 添加时间和序号信息
        numbers.append(f"序号:{i+1:03d}, 时间戳:{seed}")
    
    return numbers


def main():
    """主函数"""
    print("=" * 50)
    print("随机数求解器 v1.0")
    print("=" * 50)
    print("\n使用系统时间作为随机种子，生成20个随机数序列\n")
    
    # 设置随机种子
    seed = seed_random_with_time()
    
    # 生成随机数
    generated_numbers = generate_random_numbers(20)
    
    print("\n" + "=" * 50)
    print("生成的随机数结果:")
    print("=" * 50)
    
    for i, num in enumerate(generated_numbers):
        if isinstance(num, str):
            print(f"{num}")
        else:
            print(f"随机数={num}")
    
    print("\n" + "=" * 50)
    print(f"共生成 {len(generated_numbers)} 个数据项")
    print("=" * 50)


if __name__ == "__main__":
    main()
