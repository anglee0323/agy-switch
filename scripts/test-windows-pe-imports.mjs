import test from 'node:test';
import assert from 'node:assert/strict';
import {peImports} from './windows-pe-imports.mjs';

for (const wide of [false, true]) test(`read PE${wide ? '32+' : '32'} named and ordinal imports without execution`, () => {
  const bytes = Buffer.alloc(1024), pe = 0x80, optional = pe + 24;
  bytes.write('MZ'); bytes.writeUInt32LE(pe, 0x3c); bytes.write('PE\0\0', pe, 'binary');
  bytes.writeUInt16LE(1, pe + 6); bytes.writeUInt16LE(wide ? 240 : 224, pe + 20);
  bytes.writeUInt16LE(wide ? 0x20b : 0x10b, optional);
  bytes.writeUInt32LE(0x1000, optional + (wide ? 112 : 96) + 8);
  const section = optional + (wide ? 240 : 224);
  bytes.writeUInt32LE(512, section + 8); bytes.writeUInt32LE(0x1000, section + 12);
  bytes.writeUInt32LE(512, section + 16); bytes.writeUInt32LE(512, section + 20);
  bytes.writeUInt32LE(0x1040, 0x200); bytes.writeUInt32LE(0x1080, 0x20c);
  if (wide) { bytes.writeBigUInt64LE(0x10a0n, 0x240); bytes.writeBigUInt64LE(0x8000000000000005n, 0x248); }
  else { bytes.writeUInt32LE(0x10a0, 0x240); bytes.writeUInt32LE(0x80000005, 0x244); }
  bytes.write('kernel32.dll\0', 0x280); bytes.write('ExampleExport\0', 0x2a2);
  assert.deepEqual(peImports(bytes), [{dll: 'kernel32.dll', symbols: ['ExampleExport', 5]}]);
});
