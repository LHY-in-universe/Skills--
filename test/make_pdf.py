# -*- coding: utf-8 -*-
"""Create PDF report with Monte Carlo Art"""

import math


def read_ppm(filename):
    with open(filename, 'r') as f:
        magic = f.readline().strip()
        if magic != 'P3':
            raise ValueError('Not P3 format')
        
        while True:
            line = f.readline().strip()
            if line and not line.startswith('#'):
                break
        
        parts = line.split()
        width = int(parts[0])
        height = int(parts[1])
        
        max_val = int(f.readline().strip())
        
        pixels = []
        all_values = []
        while len(all_values) < width * height * 3:
            line = f.readline().strip()
            if line and not line.startswith('#'):
                all_values.extend(line.split())
        
        for i in range(0, len(all_values), 3):
            r = int(all_values[i])
            g = int(all_values[i+1])
            b = int(all_values[i+2])
            pixels.append((r, g, b))
        
        return pixels, width, height


def create_pdf(pixels, width, height, filename='art.pdf'):
    header = '%PDF-1.4\n'
    
    # Page content - draw image
    content = (f'q\n{width} 0 0 {height} 0 0 cm\n/Im1 Do\nQ\n').encode()
    
    # Content stream
    obj4 = (f'4 0 obj\n<< /Length {len(content)} >>\nstream\n').encode()
    obj4 += content
    obj4 += b'\nendstream\nendobj\n'
    
    # Image XObject
    img_data = b''
    for r, g, b in pixels:
        img_data += bytes([r, g, b])
    
    obj5 = (f'5 0 obj\n<< /Type /XObject /Subtype /Image\n/Width {width}\n/Height {height}'
            f'\n/ColorSpace /DeviceRGB\n/BitsPerComponent 8\n/Length {len(img_data)} >>\n'
            'stream\n').encode()
    obj5 += img_data
    obj5 += b'\nendstream\nendobj\n'
    
    # Page object
    obj3 = (f'3 0 obj\n<< /Type /Page\n/Parent 2 0 R\n/MediaBox [0 0 {width} {height}]'
            f'\n/Contents 4 0 R\n/Resources << /XObject << /Im1 5 0 R >> >> >>\nendobj\n').encode()
    
    # Pages object
    obj2 = b'2 0 obj\n<< /Type /Pages\n/Kids [3 0 R]\n/Count 1 >>\nendobj\n'
    
    # Catalog
    obj1 = b'1 0 obj\n<< /Type /Catalog\n/Pages 2 0 R >>\nendobj\n'
    
    pdf_bytes = header.encode()
    pdf_bytes += obj1
    pdf_bytes += obj2
    pdf_bytes += obj3
    pdf_bytes += obj4
    pdf_bytes += obj5
    
    # XRef table
    offsets = [
        0,
        len(header.encode()),
        len(header.encode()) + len(obj1),
        len(header.encode()) + len(obj1) + len(obj2),
        len(header.encode()) + len(obj1) + len(obj2) + len(obj3),
        len(header.encode()) + len(obj1) + len(obj2) + len(obj3) + len(obj4),
    ]
    
    xref = b'xref\n0 6\n'
    xref += b'0000000000 65535 f \n'
    for i in range(1, 6):
        xref += f'{offsets[i]:010d} 00000 n \n'.encode()
    
    trailer = (f'trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{offsets[5]}\n%%EOF\n').encode()
    
    pdf_bytes += xref
    pdf_bytes += trailer
    
    with open(filename, 'wb') as f:
        f.write(pdf_bytes)
    
    print(f'Created: {filename}')


if __name__ == '__main__':
    print('Creating PDF from PPM image...')
    print('Reading art.ppm...')
    pixels, w, h = read_ppm('art.ppm')
    print(f'Image: {w}x{h} pixels')
    print('Generating PDF...')
    create_pdf(pixels, w, h, 'art.pdf')
    print('Done! Open art.pdf to view the Monte Carlo art!')