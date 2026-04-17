# -*- coding: utf-8 -*-
"""
Python基础演示文件
"""

def main():
    print("Python基础演示")
    
    # 变量
    name = "Sam"
    age = 25
    print("Name:", name)
    print("Age:", age)
    
    # 列表
    numbers = [1, 2, 3, 4, 5]
    print("Numbers:", numbers)
    
    # 条件
    score = 85
    if score >= 80:
        print("Good score!")
    
    # 循环
    for i in range(3):
        print("Loop", i)
    
    # 函数
    def add(a, b):
        return a + b
    
    result = add(3, 5)
    print("3 + 5 =", result)
    
    print("Done!")

if __name__ == "__main__":
    main()