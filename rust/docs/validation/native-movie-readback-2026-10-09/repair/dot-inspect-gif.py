#!/usr/bin/env python3
"""Independent, fixture-focused GIF/LZW oracle. Python stdlib only.
Not a production decoder or general GIF validator. Ignores rendering/disposal
semantics beyond this fixture's opaque, noninterlaced full-canvas frames.
Usage: python inspect_gif.py [fixture.gif] > findings.json
"""
import base64
import hashlib
import json
from pathlib import Path
import sys

path = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).with_name('mirrored-once.gif')
b = path.read_bytes()
assert b[:6] in (b'GIF89a', b'GIF87a')
result = {'file': str(path), 'length': len(b), 'sha256': hashlib.sha256(b).hexdigest(), 'canvas': [int.from_bytes(b[6:8], 'little'), int.from_bytes(b[8:10], 'little')], 'frames': []}
p = 13
gpal = b[p:p+(3 << (1+(b[10]&7)))] if b[10]&128 else b''
p += len(gpal)
delay = None
while p < len(b):
    tag = b[p]
    p += 1
    if tag == 59:
        result.update(trailer_offset=p-1, exact_eof=p == len(b))
        break
    if tag == 33:
        typ = b[p]
        p += 1
        while b[p]:
            n = b[p]
            p += 1
            d = b[p:p+n]
            assert len(d) == n
            p += n
            if typ == 249:
                assert n == 4 and not (d[0] & 1), 'opaque fixture required'
                delay = int.from_bytes(d[1:3], 'little')
        p += 1
        continue
    assert tag == 44
    d = b[p:p+9]
    p += 9
    assert d[:4] == b'\0'*4 and not (d[8]&64), 'full-canvas noninterlaced fixture required'
    w = int.from_bytes(d[4:6], 'little')
    h = int.from_bytes(d[6:8], 'little')
    assert [w,h] == result['canvas']
    pal = b[p:p+(3 << (1+(d[8]&7)))] if d[8]&128 else gpal
    if d[8]&128:
        p += len(pal)
    minimum = b[p]
    assert 2 <= minimum <= 8
    p += 1
    data = bytearray()
    positions = []
    blocks = []
    while b[p]:
        n = b[p]
        p += 1
        blocks.append(n)
        assert len(b[p:p+n]) == n
        data += b[p:p+n]
        positions += range(p,p+n)
        p += n
    terminator = p
    p += 1
    clear = 1 << minimum
    eoi = clear+1
    width = minimum+1
    table = {i:bytes([i]) for i in range(clear)}
    nxt = eoi+1
    old = None
    bit = 0
    out = bytearray()
    last = None
    found_eoi = False
    while bit+width <= len(data)*8:
        start = bit
        code_width = width
        code = sum(((data[(bit+i)//8] >> ((bit+i)%8)) & 1) << i for i in range(width))
        bit += width
        if code == clear:
            table = {i:bytes([i]) for i in range(clear)}
            nxt = eoi+1
            width = minimum+1
            old = None
            continue
        if code == eoi:
            found_eoi = True
            break
        word = table[code] if code in table else old+old[:1] if code == nxt and old else None
        assert word is not None, ('invalid code',start,code,nxt)
        out += word
        assert len(out) <= w*h, 'excess decoded pixels'
        if old is not None and nxt < 4096:
            table[nxt] = old+word[:1]
            nxt += 1
            if nxt == 1 << width and width < 12:
                width += 1
        old = word
        last = {'bit_offset':start, 'width':code_width, 'code':code, 'word_length':len(word), 'total_pixels':len(out)}
    assert found_eoi, 'missing EOI'
    assert len(out) == w*h, 'wrong decoded pixel count'
    assert all(idx*3+3 <= len(pal) for idx in out), 'invalid palette index'
    rgba = bytes(v for idx in out for v in (*pal[idx*3:idx*3+3],255))
    result['frames'].append({'index':len(result['frames']), 'size':[w,h], 'delay_centiseconds':delay, 'subblock_lengths':blocks, 'minimum_code_size':minimum, 'pixel_count':len(out), 'last_pixel_code':last, 'eoi':{'bit_offset':start, 'width':code_width, 'code':code, 'file_byte_offsets':positions[start//8:(bit+7)//8]}, 'consumed_bits':bit, 'payload_bits':8*len(data), 'terminator_offset':terminator, 'indices_sha256':hashlib.sha256(out).hexdigest(), 'rgba_sha256':hashlib.sha256(rgba).hexdigest()})
assert result.get('exact_eof'), 'missing trailer or trailing bytes'
print(json.dumps(result, indent=2))
