# -*- coding: utf-8 -*-
"""Check if files exist"""

import os

def check_files():
    path = '/Users/lhy/Desktop/Skills探索/test'
    files = os.listdir(path)
    
    result = []
    for f in files:
        full = os.path.join(path, f)
        if os.path.isfile(full):
            size = os.path.getsize(full)
            result.append(f'{f}: {size} bytes')
    return result

if __name__ == '__main__':
    for item in check_files():
        print(item)