"""Genera docs/demo.bin: una memoria Mares Apneist DIMOSTRATIVA (dati inventati) per provare il visualizzatore."""
import random, sys

random.seed(7)
mem = bytearray(b"\xFF" * 0x2000)
mem[0:4] = bytes([0x41, 0x58, 0x00, 0x00])
bcd = lambda v: (v // 10) << 4 | (v % 10)
mem[0x80:0x84] = bytes([bcd(28), bcd(1), 31, 18])   # tuffo record: 1:28, 31 m, 18 °C
mem[0x84] = 45; mem[0x86] = 0; mem[0x87] = 0
mem[0x88] = 30                                      # allarme 30 m
PB, PE = 0x801, 0x1FFF

def profilo(depth, fondo):
    discesa = [min(depth, 1 + round(i * depth / max(1, depth // 2))) for i in range(max(2, depth // 2))]
    discesa[0] = random.choice([1, 2])
    risalita = [max(0, depth - round((i + 1) * depth / max(1, depth // 2 + 1))) for i in range(depth // 2 + 1)]
    p = discesa + [depth] * fondo + [max(0, x) for x in risalita]
    p[-1] = random.choice([0, 0, 1])
    return p

dives = []   # (ptr, samples)
ptr = PB
def scrivi(s):
    global ptr
    start = ptr
    for v in s:
        mem[ptr] = v
        ptr += 1
        if ptr >= PE:
            ptr = PB
    return start

# storia vecchia: abbastanza tuffi da far girare il buffer una volta
tot = 0
while tot < 6142 + 900:
    d = random.choice([3, 5, 6, 8, 10, 12, 14, 16, 18, 20, 22, 25, 28])
    s = profilo(d, random.randint(1, 6))
    dives.append((scrivi(s), s)); tot += len(s)
mem[0x800] = 0xFF; mem[0x1FFF] = 0xFF

# ultime 3 sessioni con record nel logbook
slot = 0x100
for giorno, ora, n, base in [(5, (10, 12), 14, 10), (6, (9, 40), 16, 14), (8, (11, 5), 12, 18)]:
    h, m = ora; temp = random.choice([22, 23, 24])
    for k in range(1, n + 1):
        d = max(3, min(29, base + random.randint(-6, 7)))
        s = profilo(d, random.randint(2, 8))
        p = scrivi(s)
        dur = 2 * (len(s) - 1) + random.choice([0, 1])
        mem[slot:slot + 16] = bytes([bcd(giorno), 0x01, bcd(m), bcd(h), k, 30, 0, 0, p & 0xFF, p >> 8,
                                     bcd(dur % 60), bcd(dur // 60), max(s), temp, 0xFF, 0xF4])
        slot += 16
        m += random.randint(3, 5)
        if m >= 60: m -= 60; h += 1
open(sys.argv[1], "wb").write(mem)
print("demo scritto, record:", (slot - 0x100) // 16)
