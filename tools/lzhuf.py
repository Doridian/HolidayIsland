"""LZHUF decoder (Haruhiko Okumura / Haruyasu Yoshizaki, 1988).

LZSS with a 4096-byte ring buffer (pre-filled with spaces, write position
starting at N - F), match lengths 3..60, and an adaptive Huffman tree for
literals and lengths. The upper 6 bits of a match position use a fixed
code table; bits are read MSB first. This is the classic LZHUF.C; the Holiday
Island installer archives (docs/formats/install-archive.md) store each file as
one such stream, with no size prefix.
"""

N = 4096
F = 60
THRESHOLD = 2
N_CHAR = 256 - THRESHOLD + F
T = N_CHAR * 2 - 1
R = T - 1
MAX_FREQ = 0x8000

# Upper 6 bits of the match position: (code, bit length) per value 0..63.
_P_LEN = [3] + [4] * 3 + [5] * 8 + [6] * 12 + [7] * 24 + [8] * 16
_P_CODE = [0x00, 0x20, 0x30, 0x40, 0x50, 0x58, 0x60, 0x68, 0x70, 0x78, 0x80, 0x88]
_P_CODE += list(range(0x90, 0xC0, 4)) + list(range(0xC0, 0xF0, 2)) + list(range(0xF0, 0x100))

# Inverse tables indexed by the next 8 input bits.
D_CODE = [0] * 256
D_LEN = [0] * 256
for _v, (_code, _len) in enumerate(zip(_P_CODE, _P_LEN)):
    for _j in range(1 << (8 - _len)):
        D_CODE[_code + _j] = _v
        D_LEN[_code + _j] = _len


class _Bits:
    def __init__(self, data):
        self.data = data
        self.pos = 0
        self.buf = 0
        self.n = 0

    def bit(self):
        if self.n == 0:
            self.buf = self.data[self.pos] if self.pos < len(self.data) else 0
            self.pos += 1
            self.n = 8
        self.n -= 1
        return (self.buf >> self.n) & 1

    def bits(self, k):
        v = 0
        for _ in range(k):
            v = (v << 1) | self.bit()
        return v


class _Tree:
    def __init__(self):
        self.freq = [0] * (T + 1)
        self.prnt = [0] * (T + N_CHAR)
        self.son = [0] * T
        for i in range(N_CHAR):
            self.freq[i] = 1
            self.son[i] = i + T
            self.prnt[i + T] = i
        i, j = 0, N_CHAR
        while j <= R:
            self.freq[j] = self.freq[i] + self.freq[i + 1]
            self.son[j] = i
            self.prnt[i] = self.prnt[i + 1] = j
            i += 2
            j += 1
        self.freq[T] = 0xFFFF
        self.prnt[R] = 0

    def reconst(self):
        freq, son, prnt = self.freq, self.son, self.prnt
        j = 0
        for i in range(T):
            if son[i] >= T:
                freq[j] = (freq[i] + 1) // 2
                son[j] = son[i]
                j += 1
        i, j = 0, N_CHAR
        while j < T:
            f = freq[j] = freq[i] + freq[i + 1]
            k = j - 1
            while f < freq[k]:
                k -= 1
            k += 1
            freq[k + 1 : j + 1] = freq[k:j]
            freq[k] = f
            son[k + 1 : j + 1] = son[k:j]
            son[k] = i
            i += 2
            j += 1
        for i in range(T):
            k = son[i]
            if k >= T:
                prnt[k] = i
            else:
                prnt[k] = prnt[k + 1] = i

    def update(self, c):
        freq, son, prnt = self.freq, self.son, self.prnt
        if freq[R] == MAX_FREQ:
            self.reconst()
        c = prnt[c + T]
        while True:
            freq[c] += 1
            k = freq[c]
            l = c + 1
            if k > freq[l]:
                while k > freq[l + 1]:
                    l += 1
                freq[c] = freq[l]
                freq[l] = k
                i = son[c]
                prnt[i] = l
                if i < T:
                    prnt[i + 1] = l
                j = son[l]
                son[l] = i
                prnt[j] = c
                if j < T:
                    prnt[j + 1] = c
                son[c] = j
                c = l
            c = prnt[c]
            if c == 0:
                break


def decode(data, size):
    """Decode `size` bytes from `data`. Returns (output, input bytes consumed)."""
    tree = _Tree()
    son = tree.son
    bits = _Bits(data)
    text = bytearray(b" " * N)
    r = N - F
    out = bytearray()
    while len(out) < size:
        c = son[R]
        while c < T:
            c = son[c + bits.bit()]
        c -= T
        tree.update(c)
        if c < 256:
            out.append(c)
            text[r] = c
            r = (r + 1) & (N - 1)
            continue
        i = bits.bits(8)
        hi = D_CODE[i]
        i = ((i << (D_LEN[i] - 2)) | bits.bits(D_LEN[i] - 2)) & 0xFF
        pos = (r - ((hi << 6) | (i & 0x3F)) - 1) & (N - 1)
        for k in range(c - 255 + THRESHOLD):
            ch = text[(pos + k) & (N - 1)]
            out.append(ch)
            text[r] = ch
            r = (r + 1) & (N - 1)
    return bytes(out[:size]), bits.pos
