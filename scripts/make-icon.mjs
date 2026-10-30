// Generates icon-source.png (1024x1024, pixel-art) without external deps.
// Then: `npx tauri icon icon-source.png -o src-tauri/icons`.
import { deflateSync } from 'node:zlib'
import { writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..')
const GRID = 32
const SCALE = 32
const SIZE = GRID * SCALE

const BG = [0x12, 0x12, 0x1c, 255]
const BORDER = [0xff, 0xd2, 0x3f, 255]
const INNER = [0x1d, 0x1d, 0x2e, 255]
const ACCENT = [0x4a, 0xde, 0x80, 255]
const SHADOW = [0x05, 0x05, 0x0a, 255]

const px = Array.from({ length: GRID }, () => Array.from({ length: GRID }, () => [0, 0, 0, 0]))
const set = (x, y, c) => {
  if (x >= 0 && y >= 0 && x < GRID && y < GRID) px[y][x] = c
}
const rect = (x0, y0, x1, y1, c) => {
  for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) set(x, y, c)
}

// hard drop shadow
rect(6, 6, 28, 28, SHADOW)
// frame
rect(3, 3, 25, 25, BORDER)
rect(5, 5, 23, 23, INNER)
// inner scanline texture
for (let y = 7; y <= 21; y += 4) rect(6, y, 22, y, [0x26, 0x26, 0x40, 255])
// play triangle (pixel)
const tri = [
  [11, 10], [11, 11], [11, 12], [11, 13], [11, 14], [11, 15], [11, 16], [11, 17],
  [12, 11], [12, 12], [12, 13], [12, 14], [12, 15], [12, 16],
  [13, 12], [13, 13], [13, 14], [13, 15],
  [14, 13], [14, 14]
]
for (const [x, y] of tri) set(x, y, ACCENT)
// corner rivets
for (const [x, y] of [[6, 6], [22, 6], [6, 22], [22, 22]]) set(x, y, BORDER)

// scale
const out = Buffer.alloc(SIZE * SIZE * 4)
for (let y = 0; y < SIZE; y++) {
  for (let x = 0; x < SIZE; x++) {
    const c = px[Math.floor(y / SCALE)][Math.floor(x / SCALE)]
    const i = (y * SIZE + x) * 4
    out[i] = c[0]
    out[i + 1] = c[1]
    out[i + 2] = c[2]
    out[i + 3] = c[3]
  }
}

function crc32(buf) {
  let c = 0xffffffff
  for (const b of buf) {
    c ^= b
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
  }
  return (c ^ 0xffffffff) >>> 0
}

function chunk(type, data) {
  const len = Buffer.alloc(4)
  len.writeUInt32BE(data.length)
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data])
  const crc = Buffer.alloc(4)
  crc.writeUInt32BE(crc32(body))
  return Buffer.concat([len, body, crc])
}

const ihdr = Buffer.alloc(13)
ihdr.writeUInt32BE(SIZE, 0)
ihdr.writeUInt32BE(SIZE, 4)
ihdr[8] = 8 // bit depth
ihdr[9] = 6 // RGBA
const raw = Buffer.alloc((SIZE * 4 + 1) * SIZE)
for (let y = 0; y < SIZE; y++) {
  raw[y * (SIZE * 4 + 1)] = 0
  out.copy(raw, y * (SIZE * 4 + 1) + 1, y * SIZE * 4, (y + 1) * SIZE * 4)
}

const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk('IHDR', ihdr),
  chunk('IDAT', deflateSync(raw, { level: 9 })),
  chunk('IEND', Buffer.alloc(0))
])

const target = join(ROOT, 'icon-source.png')
writeFileSync(target, png)
console.log(`wrote ${target} (${SIZE}x${SIZE}, ${png.length} bytes)`)
