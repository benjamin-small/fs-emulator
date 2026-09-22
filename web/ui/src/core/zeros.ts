/** One byte per sector: 1 when every byte of the sector is zero. */
export function scanZeroSectors(image: Uint8Array, sectorSize: number): Uint8Array {
  const count = Math.floor(image.length / sectorSize);
  const bits = new Uint8Array(count);
  for (let s = 0; s < count; s++) bits[s] = isZero(image, sectorSize, s) ? 1 : 0;
  return bits;
}

export function rescanSectors(bits: Uint8Array, image: Uint8Array, sectorSize: number, sectors: Iterable<number>): void {
  for (const s of sectors) if (s >= 0 && s < bits.length) bits[s] = isZero(image, sectorSize, s) ? 1 : 0;
}

function isZero(image: Uint8Array, sectorSize: number, sector: number): boolean {
  const start = sector * sectorSize;
  const aligned = (image.byteOffset + start) % 4 === 0 && sectorSize % 4 === 0;
  if (aligned) {
    const words = new Uint32Array(image.buffer, image.byteOffset + start, sectorSize / 4);
    for (let i = 0; i < words.length; i++) if (words[i] !== 0) return false;
    return true;
  }
  for (let i = start; i < start + sectorSize; i++) if (image[i] !== 0) return false;
  return true;
}
