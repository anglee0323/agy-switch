import fs from 'node:fs';
import {pathToFileURL} from 'node:url';

// Inspect imports without loading or executing the PE file.
export function peImports(bytes) {
  const pe = bytes.readUInt32LE(0x3c);
  if (bytes.toString('binary', pe, pe + 4) !== 'PE\0\0') throw new Error('Expected PE file');
  const optional = pe + 24, magic = bytes.readUInt16LE(optional);
  if (magic !== 0x20b && magic !== 0x10b) throw new Error('Unsupported PE optional header');
  const wide = magic === 0x20b, sections = optional + bytes.readUInt16LE(pe + 20);
  const offset = rva => {
    for (let i = 0; i < bytes.readUInt16LE(pe + 6); i++) {
      const section = sections + i * 40, start = bytes.readUInt32LE(section + 12);
      const size = Math.max(bytes.readUInt32LE(section + 8), bytes.readUInt32LE(section + 16));
      if (rva >= start && rva < start + size) return bytes.readUInt32LE(section + 20) + rva - start;
    }
    throw new Error(`Unmapped PE RVA ${rva}`);
  };
  const string = at => {
    const end = bytes.indexOf(0, at);
    if (end < 0) throw new Error('Unterminated PE import name');
    return bytes.toString('ascii', at, end);
  };
  const imports = bytes.readUInt32LE(optional + (wide ? 112 : 96) + 8);
  if (!imports) return [];
  const result = [];
  for (let descriptor = offset(imports); bytes.readUInt32LE(descriptor + 12); descriptor += 20) {
    const dll = string(offset(bytes.readUInt32LE(descriptor + 12)));
    const thunk = offset(bytes.readUInt32LE(descriptor) || bytes.readUInt32LE(descriptor + 16));
    const symbols = [];
    for (let at = thunk; ; at += wide ? 8 : 4) {
      const value = wide ? bytes.readBigUInt64LE(at) : BigInt(bytes.readUInt32LE(at));
      if (value === 0n) break;
      const ordinal = wide ? 1n << 63n : 1n << 31n;
      symbols.push(value & ordinal ? Number(value & 0xffffn) : string(offset(Number(value)) + 2));
    }
    result.push({dll, symbols});
  }
  return result;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  console.log(JSON.stringify(peImports(fs.readFileSync(process.argv[2]))));
}
