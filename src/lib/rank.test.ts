import { describe, it, expect } from 'vitest'
import { matchScore, frecencyBonus, groupResults, type HistoryEntry } from './rank'

const app = (id: string, name: string, keywords: string[] = []) => ({ id, name, keywords })

describe('matchScore', () => {
  it('exact > prefix > word-boundary > substring', () => {
    expect(matchScore('firefox', app('a', 'Firefox'))).toBe(120)
    expect(matchScore('fire', app('a', 'Firefox'))).toBe(90)
    expect(matchScore('fox', app('a', 'Fire fox'))).toBe(60)
    expect(matchScore('ire', app('a', 'Firefox'))).toBe(30)
    expect(matchScore('zzz', app('a', 'Firefox'))).toBeNull()
  })
  it('matches keywords', () => {
    expect(matchScore('browser', app('a', 'Firefox', ['browser', 'web']))).toBe(80)
  })
  it('empty query matches everything with 0', () => {
    expect(matchScore('  ', app('a', 'Firefox'))).toBe(0)
  })
})

describe('frecencyBonus', () => {
  const now = 1_000_000
  const mk = (ts: number, count: number): HistoryEntry => ({ id: 'x', name: 'x', kind: 'app', ts, count })
  it('no history → 0', () => expect(frecencyBonus(undefined, now)).toBe(0))
  it('recent use beats old use', () => {
    expect(frecencyBonus(mk(now - 100, 1), now)).toBeGreaterThan(frecencyBonus(mk(now - 40 * 86400, 1), now))
  })
  it('frequency caps at 15 uses', () => {
    expect(frecencyBonus(mk(now - 100, 1000), now)).toBeLessThanOrEqual(30 + 38)
  })
})

describe('groupResults', () => {
  const now = 1_000_000
  const apps = [app('app:a', 'Alpha'), app('app:b', 'Beta'), app('app:g', 'Gamma')]
  it('sorts by score and flattens', () => {
    const favs = new Set<string>(['app:g'])
    const { flat } = groupResults([{ title: 'Apps', candidates: apps }], 'a', [], favs, now)
    expect(flat[0].id).toBe('app:g') // favorite boost wins
  })
  it('non-matching excluded, empty sections dropped', () => {
    const { sections } = groupResults([{ title: 'Apps', candidates: apps }], 'zzz', [], new Set(), now)
    expect(sections).toHaveLength(0)
  })
})
