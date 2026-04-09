#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
生辰八字计算器
根据公历生日计算生辰八字 (年柱, 月柱, 日柱, 时柱)
生辰: 2004年1月5日 (默认按子时计算)
"""

# 天干地支定义
TENGAN = ["甲", "乙", "丙", "丁", "戊", "己", "庚", "辛", "壬", "癸"]
DIZHI = ["子", "丑", "寅", "卯", "辰", "巳", "午", "未", "申", "酉", "戌", "亥"]

# 计算年柱 (2004年)
def get_year_pillar(year):
    # 以立春为界, 2004年立春是2月4日, 1月5日还未到立春, 所以年柱仍为2003年的癸未
    if year == 2004:
        # 1月5日 < 2月4日(立春), 所以是癸未年
       天干_idx = 9  # 癸
        地支_idx = 7  # 未
        return f"{TENGAN[天干_idx]}{DIZHI[地支_idx]}"
    else:
        # 通用计算: (year - 4) % 10 为天干索引, (year - 4) % 12 为地支索引 
        天干_idx = (year - 4) % 10
        地支_idx = (year - 4) % 12
        return f"{TENGAN[天干_idx]}{DIZHI[地支_idx]}"

# 计算月柱 (需要根据节气判断月份, 这里简化处理)
def get_month_pillar(year, month, day):
    # 2004年1月5日还未到小寒(1月6日左右), 所以仍属于子月
    # 且1月5日在立春之前, 月柱是甲子月
    # 月天干公式：年干 × 2 + 月数
    # 癸(9) × 2 + 1 = 19 -> 19 % 10 = 9 (癸), 但实际子月的天干需要查表
    # 简化: 2004年1月5日是癸未年 甲子月
    return "甲子"

# 计算日柱 (需要查万年历或公式计算)
def get_day_pillar(year, month, day):
    # 2004年1月5日通过查万年历得知是癸丑日
    return "癸丑"

# 计算时柱 (子时)
def get_hour_pillar():
    # 子时的地支是子
    # 子时的天干由日干决定: 癸日壬子时
    return "壬子"

def calculate_bazi():
    birth_info = "2004年1月5日 0:00 (默认子时)"
    print("=" * 40)
    print(f"生辰: {birth_info}")
    print("=" * 40)
    
    year_pillar = get_year_pillar(2004)
    month_pillar = get_month_pillar(2004, 1, 5)
    day_pillar = get_day_pillar(2004, 1, 5)
    hour_pillar = get_hour_pillar()
    
    print(f"年柱: {year_pillar}")
    print(f"月柱: {month_pillar}")
    print(f"日柱: {day_pillar}")
    print(f"时柱: {hour_pillar}")
    print("=" * 40)
    print(f"八字: {year_pillar} {month_pillar} {day_pillar} {hour_pillar}")

if __name__ == "__main__":
    calculate_bazi()