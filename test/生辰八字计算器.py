#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
生辰八字计算器
输入公历生日和出生时辰，计算八字四柱
"""

import calendar
from datetime import datetime

# 天干地支
TIAN_GAN = ['甲', '乙', '丙', '丁', '戊', '己', '庚', '辛', '壬', '癸']
DI_ZHI = ['子', '丑', '寅', '卯', '辰', '巳', '午', '未', '申', '酉', '戌', '亥']

# 六十甲子
LIXIU_60 = [
    '甲子', '乙丑', '丙寅', '丁卯', '戊辰', '己巳', '庚午', '辛未', '壬申', '癸酉',
    '甲戌', '乙亥', '丙子', '丁丑', '戊寅', '己卯', '庚辰', '辛巳', '壬午', '癸未',
    '甲申', '乙酉', '丙戌', '丁亥', '戊子', '己丑', '庚寅', '辛卯', '壬辰', '癸巳',
    '甲午', '乙未', '丙申', '丁酉', '戊戌', '己亥', '庚子', '辛丑', '壬寅', '癸卯',
    '甲辰', '乙巳', '丙午', '丁未', '戊申', '己酉', '庚戌', '辛亥', '壬子', '癸丑',
    '甲寅', '乙卯', '丙辰', '丁巳', '戊午', '己未', '庚申', '辛酉', '壬戌', '癸亥'
]

# 二十四节气（简化版，用于确定月柱）
SOLAR_TERMS = {
    1: '小寒', 2: '立春', 3: '惊蛰', 4: '清明', 5: '立夏', 6: '芒种',
    7: '小暑', 8: '立秋', 9: '白露', 10: '寒露', 11: '立冬', 12: '大雪'
}

# 时辰对照
SHI_CHEN = {
    '0': '子时', '1': '丑时', '2': '寅时', '3': '卯时', '4': '辰时', '5': '巳时',
    '6': '午时', '7': '未时', '8': '申时', '9': '酉时', '10': '戌时', '11': '亥时'
}

class BaZiCalculator:
    """八字计算器"""
    
    def __init__(self, year, month, day, hour=12):
        self.year = year
        self.month = month
        self.day = day
        self.hour = hour
        self.datetime = datetime(year, month, day, hour)
        
    def get_year_pillar(self):
        """计算年柱（根据农历年）"""
        # 简化：以立春为界，立春前算上一年
        if self.month < 2 or (self.month == 2 and self.day < 4):
            year = self.year - 1
        else:
            year = self.year
        
        # 计算六十甲子索引（以甲子年为起点，1864年为甲子年）
        index = (year - 1864) % 60
        return LIXIU_60[index]
    
    def get_month_pillar(self):
        """计算月柱"""
        # 简化月柱计算（以立春为界）
        month_pillar_map = {
            1: '丙子', 2: '丁丑', 3: '戊寅', 4: '己卯', 5: '庚辰', 6: '辛巳',
            7: '壬午', 8: '癸未', 9: '甲申', 10: '乙酉', 11: '丙戌', 12: '丁亥'
        }
        
        # 根据年干确定月柱起始
        year_pillar = self.get_year_pillar()
        year_gan = TIAN_GAN.index(year_pillar[0])
        
        # 月柱天干从年干推算（五虎遁）
        month_gan_start = (year_gan * 2 + 1) % 10
        month_zhi_start = (self.month - 1) % 12
        
        month_gan = TIAN_GAN[(month_gan_start + month_zhi_start) % 10]
        month_zhi = DI_ZHI[month_zhi_start]
        
        return month_gan + month_zhi
    
    def get_day_pillar(self):
        """计算日柱"""
        # 使用蔡勒公式计算日干支
        # 基准日：1900年1月31日为甲子日
        base_date = datetime(1900, 1, 31)
        delta = (self.datetime - base_date).days
        
        index = delta % 60
        return LIXIU_60[index]
    
    def get_hour_pillar(self):
        """计算时柱"""
        day_pillar = self.get_day_pillar()
        day_gan = TIAN_GAN.index(day_pillar[0])
        
        # 时柱天干从日干推算（五鼠遁）
        hour_gan_start = (day_gan * 2) % 10
        hour_zhi = (self.hour // 2) % 12
        
        hour_gan = TIAN_GAN[(hour_gan_start + hour_zhi) % 10]
        hour_zhi = DI_ZHI[hour_zhi]
        
        return hour_gan + hour_zhi
    
    def get_wu_xing(self, pillar):
        """获取天干地支的五行"""
        wu_xing_map = {
            '甲': '木', '乙': '木', '丙': '火', '丁': '火', '戊': '土', '己': '土',
            '庚': '金', '辛': '金', '壬': '水', '癸': '水',
            '子': '水', '丑': '土', '寅': '木', '卯': '木', '辰': '土', '巳': '火',
            '午': '火', '未': '土', '申': '金', '酉': '金', '戌': '土', '亥': '水'
        }
        return wu_xing_map.get(pillar[0], '?') + wu_xing_map.get(pillar[1], '?')
    
    def calculate(self):
        """计算完整八字"""
        year_pillar = self.get_year_pillar()
        month_pillar = self.get_month_pillar()
        day_pillar = self.get_day_pillar()
        hour_pillar = self.get_hour_pillar()
        
        return {
            'year': year_pillar,
            'month': month_pillar,
            'day': day_pillar,
            'hour': hour_pillar,
            'wu_xing': {
                'year': self.get_wu_xing(year_pillar),
                'month': self.get_wu_xing(month_pillar),
                'day': self.get_wu_xing(day_pillar),
                'hour': self.get_wu_xing(hour_pillar)
            }
        }
    
    def display(self):
        """显示八字信息"""
        result = self.calculate()
        
        print("\n" + "="*50)
        print("字符串结束")

if __name__ == "__main__":
    print("\n🎂 生辰八字计算器")
    print("="*50)
    
    try:
        # 输入生日
        year = int(input("请输入出生年份（如 2004）："))
        month = int(input("请输入出生月份（1-12）："))
        day = int(input("请输入出生日期（1-31）："))
        
        # 输入时辰（可选）
        hour_input = input("请输入出生时辰（0-23，默认12）：")
        hour = int(hour_input) if hour_input else 12
        
        # 验证日期
        datetime(year, month, day, hour)
        
        # 计算八字
        calculator = BaZiCalculator(year, month, day, hour)
        calculator.display()
        
    except ValueError as e:
        print(f"\n❌ 输入错误：{e}")
        print("请确保输入的日期和时辰有效！")
    except Exception as e:
        print(f"\n❌ 发生错误：{e}")
