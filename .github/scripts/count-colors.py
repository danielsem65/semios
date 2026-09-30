"""Count distinct colours in a PNG without third-party imaging libraries.

Used by the Android smoke test to tell a rendered start page from a blank
window: a live process is not evidence that anything was drawn. Samples on a
coarse grid, which is enough to separate a flat background from real content
and keeps a 1080x2400 screenshot cheap to decode.
"""

import struct
import sys
import zlib

CHANNELS = {0: 1, 2: 3, 4: 2, 6: 4}
STEP = 8


def paeth(a: int, b: int, c: int) -> int:
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    return b if pb <= pc else c


def main() -> int:
    data = open(sys.argv[1], "rb").read()
    pos, idat, width, height, color_type = 8, b"", 0, 0, 0

    while pos < len(data):
        length = struct.unpack(">I", data[pos : pos + 4])[0]
        kind = data[pos + 4 : pos + 8]
        body = data[pos + 8 : pos + 8 + length]
        if kind == b"IHDR":
            width, height, _, color_type = struct.unpack(">IIBB", body[:10])
        elif kind == b"IDAT":
            idat += body
        pos += 12 + length

    channels = CHANNELS.get(color_type, 3)
    raw = zlib.decompress(idat)
    stride = width * channels
    seen: set[bytes] = set()
    previous = bytearray(stride)
    offset = 0

    for y in range(height):
        filter_type = raw[offset]
        offset += 1
        line = bytearray(raw[offset : offset + stride])
        offset += stride

        for x in range(stride):
            left = line[x - channels] if x >= channels else 0
            up = previous[x]
            upleft = previous[x - channels] if x >= channels else 0
            if filter_type == 1:
                line[x] = (line[x] + left) & 255
            elif filter_type == 2:
                line[x] = (line[x] + up) & 255
            elif filter_type == 3:
                line[x] = (line[x] + (left + up) // 2) & 255
            elif filter_type == 4:
                line[x] = (line[x] + paeth(left, up, upleft)) & 255

        if y % STEP == 0:
            for x in range(0, stride, channels * STEP):
                seen.add(bytes(line[x : x + 3]))

        previous = line

    print(len(seen))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
