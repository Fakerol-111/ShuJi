import { describe, expect, it } from 'vitest';
import { basenameFromPath, splitPathParts } from './pathBasename';

describe('basenameFromPath', () => {
  it('handles unix paths', () => {
    expect(basenameFromPath('.shuji/designs/foo.md')).toBe('foo.md');
    expect(basenameFromPath('/home/user/project/src/main.rs')).toBe('main.rs');
  });

  it('handles windows paths', () => {
    expect(basenameFromPath('C:\\Program Files\\project\\file.rs')).toBe('file.rs');
    expect(basenameFromPath('.shuji\\reviews\\bar.md')).toBe('bar.md');
  });

  it('returns original string when no separator', () => {
    expect(basenameFromPath('filename.txt')).toBe('filename.txt');
  });
});

describe('splitPathParts', () => {
  it('splits unix paths', () => {
    expect(splitPathParts('.shuji/designs/foo.md')).toEqual(['.shuji', 'designs', 'foo.md']);
  });

  it('splits windows paths', () => {
    expect(splitPathParts('.shuji\\designs\\foo.md')).toEqual(['.shuji', 'designs', 'foo.md']);
  });
});
