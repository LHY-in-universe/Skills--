# -*- coding: utf-8 -*-
"""Monte Carlo Art Generator using random numbers"""

import random
import math


def generate_art(width=300, height=200, points=8000, seed=42):
    random.seed(seed)
    pixels = [[(0, 0, 0) for _ in range(width)] for _ in range(height)]
    
    print("Generating", points, "points...")
    
    for _ in range(points):
        x = random.randint(0, width - 1)
        y = random.randint(0, height - 1)
        xn = x / float(width)
        yn = y / float(height)
        
        # Gradients using trig functions
        r = int(255 * (0.5 + 0.5 * math.sin(2 * 3.14159 * xn)))
        g = int(255 * (0.5 + 0.5 * math.cos(2 * 3.14159 * yn)))
        b = int(255 * (0.5 + 0.5 * math.sin(2 * 3.14159 * (xn + yn))))
        
        # Radial pattern
        cx, cy = 0.5, 0.5
        dist = math.sqrt((xn - cx) ** 2 + (yn - cy) ** 2)
        ring = 0.5 + 0.5 * math.sin(10 * 3.14159 * dist)
        
        r = int(r * ring)
        g = int(g * ring)
        b = int(b * ring)
        
        # Draw 2x2 point
        for dx in [0, 1]:
            for dy in [0, 1]:
                px, py = x + dx, y + dy
                if px < width and py < height:
                    pixels[py][px] = (r, g, b)
    
    return pixels


def save_ppm(pixels, name='art.ppm'):
    h = len(pixels)
    w = len(pixels[0])
    
    f = open(name, 'w')
    f.write('P3\n')
    f.write(str(w) + ' ' + str(h) + '\n')
    f.write('255\n')
    
    for row in pixels:
        line_parts = []
        for (r, g, b) in row:
            line_parts.append(str(r) + ' ' + str(g) + ' ' + str(b))
        f.write(' '.join(line_parts) + '\n')
    
    f.close()
    return name


def show_preview(pixels):
    chars = ' .:-=+*#%@'
    h = len(pixels)
    w = len(pixels[0])
    
    print('')
    print('Preview:')
    for y in range(0, h, 2):
        line = ''
        for x in range(0, w, 2):
            r, g, b = pixels[y][x]
            bright = (r + g + b) / 765.0
            idx = int(bright * (len(chars) - 1))
            line += chars[idx]
        print(line)
    print('')


def main():
    print('=' * 45)
    print('   Monte Carlo Art Generator')
    print('=' * 45)
    
    art = generate_art(300, 200, 12000)
    name = save_ppm(art)
    print('Saved:', name)
    show_preview(art)
    print('Done!')


if __name__ == '__main__':
    main()