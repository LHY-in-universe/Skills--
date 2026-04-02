#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
函数图像绘制器
绘制多种常见函数的图像并保存为图片文件
"""

import numpy as np
import matplotlib.pyplot as plt
import os


def plot_functions():
    """
    绘制多种函数的图像
    """
    # 设置中文字体（避免乱码）
    plt.rcParams['font.sans-serif'] = ['Arial Unicode MS', 'SimHei', 'DejaVu Sans']
    plt.rcParams['axes.unicode_minus'] = False
    
    # 创建画布
    fig, axes = plt.subplots(2, 2, figsize=(12, 10))
    fig.suptitle('Common Function Plots', fontsize=16, fontweight='bold')
    
    # 定义 x 范围
    x = np.linspace(-2 * np.pi, 2 * np.pi, 1000)
    
    # 子图 1: sin(x)
    ax1 = axes[0, 0]
    ax1.plot(x, np.sin(x), 'b-', linewidth=2, label='sin(x)')
    ax1.axhline(y=0, color='k', linestyle='--', alpha=0.3)
    ax1.axvline(x=0, color='k', linestyle='--', alpha=0.3)
    ax1.set_title('Sine Function: sin(x)', fontsize=12)
    ax1.set_xlabel('x')
    ax1.set_ylabel('y')
    ax1.legend()
    ax1.grid(True, alpha=0.3)
    
    # 子图 2: cos(x)
    ax2 = axes[0, 1]
    ax2.plot(x, np.cos(x), 'r-', linewidth=2, label='cos(x)')
    ax2.axhline(y=0, color='k', linestyle='--', alpha=0.3)
    ax2.axvline(x=0, color='k', linestyle='--', alpha=0.3)
    ax2.set_title('Cosine Function: cos(x)', fontsize=12)
    ax2.set_xlabel('x')
    ax2.set_ylabel('y')
    ax2.legend()
    ax2.grid(True, alpha=0.3)
    
    # 子图 3: x^2
    ax3 = axes[1, 0]
    x3 = np.linspace(-3, 3, 500)
    ax3.plot(x3, x3**2, 'g-', linewidth=2, label='x^2')
    ax3.axhline(y=0, color='k', linestyle='--', alpha=0.3)
    ax3.axvline(x=0, color='k', linestyle='--', alpha=0.3)
    ax3.set_title('Quadratic Function: x^2', fontsize=12)
    ax3.set_xlabel('x')
    ax3.set_ylabel('y')
    ax3.legend()
    ax3.grid(True, alpha=0.3)
    
    # 子图 4: e^x
    ax4 = axes[1, 1]
    x4 = np.linspace(-3, 3, 500)
    ax4.plot(x4, np.exp(x4), 'm-', linewidth=2, label='e^x')
    ax4.axhline(y=0, color='k', linestyle='--', alpha=0.3)
    ax4.axvline(x=0, color='k', linestyle='--', alpha=0.3)
    ax4.set_title('Exponential Function: e^x', fontsize=12)
    ax4.set_xlabel('x')
    ax4.set_ylabel('y')
    ax4.legend()
    ax4.grid(True, alpha=0.3)
    
    # 调整布局
    plt.tight_layout()
    
    # 保存图像
    output_dir = '/Users/lhy/Desktop/Skills探索/test'
    output_file = os.path.join(output_dir, 'function_plot.png')
    plt.savefig(output_file, dpi=150, bbox_inches='tight')
    print('Image saved to:', output_file)
    
    # 显示图像
    plt.show()
    
    return output_file


def main():
    """Main function"""
    print('=' * 50)
    print('Function Plotter v1.0')
    print('=' * 50)
    print('')
    print('Plotting the following functions:')
    print('  • sin(x) - Sine function')
    print('  • cos(x) - Cosine function')
    print('  • x^2    - Quadratic function')
    print('  • e^x    - Exponential function')
    print('')
    print('=' * 50)
    
    image_path = plot_functions()
    
    print('')
    print('=' * 50)
    print('Image file:', image_path)
    print('=' * 50)


if __name__ == "__main__":
    main()