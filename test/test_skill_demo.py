#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
技能测试演示脚本 - Test Skill Demo
作者: ClawTest (为 Sam 创建)
功能: 展示多种Python编程技能的综合应用
"""

import random
import math
import time
from datetime import datetime
from typing import List, Dict


class SkillDemo:
    """技能演示类"""
    
    def __init__(self):
        self.name = "Sam"
        self.location = "深圳"
        self.occupation = "AI"
        
    def greet(self) -> str:
        """生成个性化问候"""
        hour = datetime.now().hour
        if 5 <= hour < 12:
            greeting = "早上好"
        elif 12 <= hour < 18:
            greeting = "下午好"
        else:
            greeting = "晚上好"
        return f"{greeting}, {self.name}! 欢迎来自 {self.location} 的 {self.occupation} 从业者！"
    
    def random_data_generator(self, count: int = 10) -> List[Dict]:
        """生成随机测试数据"""
        data = []
        for i in range(count):
            record = {
                "id": i + 1,
                "value": round(random.uniform(1, 100), 2),
                "category": random.choice(["A", "B", "C", "D"]),
                "timestamp": datetime.now().strftime("%Y-%m-%d %H:%M:%S")
            }
            data.append(record)
        return data
    
    def analyze_data(self, data: List[Dict]) -> Dict:
        """分析数据并返回统计信息"""
        if not data:
            return {}
        
        values = [item["value"] for item in data]
        categories = [item["category"] for item in data]
        
        stats = {
            "total_count": len(data),
            "average_value": round(sum(values) / len(values), 2),
            "max_value": max(values),
            "min_value": min(values),
            "sum_value": round(sum(values), 2),
            "category_distribution": {
                cat: categories.count(cat) for cat in set(categories)
            }
        }
        return stats
    
    def fibonacci_sequence(self, n: int) -> List[int]:
        """生成斐波那契数列"""
        if n <= 0:
            return []
        elif n == 1:
            return [0]
        
        sequence = [0, 1]
        for i in range(2, n):
            sequence.append(sequence[i-1] + sequence[i-2])
        return sequence
    
    def monte_carlo_pi(self, iterations: int = 10000) -> float:
        """使用蒙特卡洛方法估算π值"""
        inside_circle = 0
        
        for _ in range(iterations):
            x = random.uniform(0, 1)
            y = random.uniform(0, 1)
            if x**2 + y**2 <= 1:
                inside_circle += 1
        
        pi_estimate = 4 * inside_circle / iterations
        return pi_estimate
    
    def prime_checker(self, num: int) -> bool:
        """检查是否为质数"""
        if num < 2:
            return False
        if num == 2:
            return True
        if num % 2 == 0:
            return False
        
        for i in range(3, int(math.sqrt(num)) + 1, 2):
            if num % i == 0:
                return False
        return True
    
    def run_all_tests(self):
        """运行所有测试功能"""
        print("=" * 60)
        print("🎯 技能测试演示脚本 - 开始运行")
        print("=" * 60)
        print()
        
        # 1. 问候功能
        print("1️⃣ 个性化问候:")
        print(f"   {self.greet()}")
        print()
        
        # 2. 随机数据生成
        print("2️⃣ 随机数据生成 (10条):")
        data = self.random_data_generator(10)
        for item in data[:5]:  # 只显示前5条
            print(f"   ID:{item['id']} | 值:{item['value']} | 分类:{item['category']}")
        print(f"   ... 共生成 {len(data)} 条数据")
        print()
        
        # 3. 数据分析
        print("3️⃣ 数据统计分析:")
        stats = self.analyze_data(data)
        print(f"   总记录数: {stats['total_count']}")
        print(f"   平均值: {stats['average_value']}")
        print(f"   最大值: {stats['max_value']}")
        print(f"   最小值: {stats['min_value']}")
        print(f"   总和: {stats['sum_value']}")
        print(f"   分类分布: {stats['category_distribution']}")
        print()
        
        # 4. 斐波那契数列
        print("4️⃣ 斐波那契数列 (前15项):")
        fib = self.fibonacci_sequence(15)
        print(f"   {fib}")
        print()
        
        # 5. 蒙特卡洛估算π
        print("5️⃣ 蒙特卡洛方法估算π值:")
        pi_est = self.monte_carlo_pi(10000)
        print(f"   估算值: {pi_est:.6f}")
        print(f"   真实值: {math.pi:.6f}")
        print(f"   误差: {abs(math.pi - pi_est):.6f}")
        print()
        
        # 6. 质数检查
        print("6️⃣ 质数检查 (1-20):")
        primes = [i for i in range(1, 21) if self.prime_checker(i)]
        print(f"   质数列表: {primes}")
        print()
        
        print("=" * 60)
        print("✅ 所有测试完成!")
        print("=" * 60)


def main():
    """主函数"""
    print(f"🚀 脚本启动时间: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print()
    
    demo = SkillDemo()
    demo.run_all_tests()
    
    print()
    print(f"🏁 脚本结束时间: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")


if __name__ == "__main__":
    main()
