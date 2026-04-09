# -*- coding: utf-8 -*-
"""
获取计算机状态信息的脚本
"""

import platform
import os
import sys


def main():
    print("=" * 50)
    print("计算机状态信息")
    print("=" * 50)

    # 系统信息
    sys_name = platform.system()
    sys_version = platform.version()
    print(f"操作系统：{sys_name} {sys_version}")

    # 获取主目录
    print(f"用户主目录：{os.path.expanduser('~')}")

    # 当前工作目录
    print(f"当前工作目录：{os.getcwd()}")

    # 磁盘空间
    try:
        import shutil
        usage = shutil.disk_usage(os.getcwd())
        total_gb = usage.total // (1024**3)
        used_gb = usage.used // (1024**3)
        free_gb = usage.free // (1024**3)
        print(f"磁盘总大小：{total_gb} GB")
        print(f"已使用：{used_gb:.2f} GB")
        print(f"剩余：{free_gb:.2f} GB")
        print(f"使用率：{usage.usage_percent:.2f}%")
    except Exception as e:
        print(f"磁盘信息获取失败：{e}")

    print("=" * 50)


if __name__ == '__main__':
    main()