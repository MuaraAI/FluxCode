"""Compress one file as .xz for FluxCode's updater: python packaging/xz.py IN OUT

LZMA2 at preset 9 extreme with a CRC64 check (src/update.rs reads it with lzma-rs and
refuses a damaged download). Python's own lzma module, so every CI runner has it.
"""
import lzma
import sys

src, dst = sys.argv[1], sys.argv[2]
with open(src, "rb") as f:
    data = f.read()
packed = lzma.compress(data, format=lzma.FORMAT_XZ, check=lzma.CHECK_CRC64, preset=9 | lzma.PRESET_EXTREME)
# Read it back before it is published.
assert lzma.decompress(packed) == data
with open(dst, "wb") as f:
    f.write(packed)
print(f"{dst}: {len(data) / 2**20:.2f} MB -> {len(packed) / 2**20:.2f} MB")
