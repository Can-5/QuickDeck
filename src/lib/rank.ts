// Ranking for the launcher: text match quality × usage frecency × favorite boost.
// Pure and unit-tested; all data comes from the backend snapshot + app list.

export interface RankableItem {
  id: string
  name: string
  keywords?: string[]
}

export interface HistoryEntry {
  id: string
  name: string
  kind: string
  ts: number
  count: number
}

export interface RankedItem<T extends RankableItem = RankableItem> {
  item: T
  score: number
  section: string
}

/** Score how well `q` matches `item`. Returns null when there is no match. */
export function matchScore(qRaw: string, item: RankableItem): number | null {
  const q = qRaw.trim().toLowerCase()
  if (!q) return 0
  const name = item.name.toLowerCase()
  let best = -1
  if (name === q) best = 120
  else if (name.startsWith(q)) best = 90
  else if (name.split(/[\s\-_/().]+/).some((w) => w.startsWith(q))) best = 60
  else if (name.includes(q)) best = 30
  for (const kw of item.keywords ?? []) {
    const k = kw.toLowerCase()
    if (k === q) best = Math.max(best, 80)
    else if (k.startsWith(q)) best = Math.max(best, 55)
    else if (k.includes(q)) best = Math.max(best, 12)
  }
  return best < 0 ? null : best
}

/** Usage-based bonus in [0, ~70]. */
export function frecencyBonus(historyEntry: HistoryEntry | undefined, nowSec: number): number {
  if (!historyEntry) return 0
  const ageSec = Math.max(0, nowSec - historyEntry.ts)
  let recency = 0
  if (ageSec < 24 * 3600) recency = 30
  else if (ageSec < 7 * 24 * 3600) recency = 20
  else if (ageSec < 30 * 24 * 3600) recency = 10
  const frequency = Math.min(historyEntry.count, 15) * 2.5
  return Math.round(recency + frequency)
}

/** Combined score: base match + frecency + favorite boost (1000). */
export function rankItem<T extends RankableItem>(
  q: string,
  item: T,
  history: HistoryEntry[],
  favorites: Set<string>,
  nowSec: number
): number | null {
  const base = matchScore(q, item)
  if (base === null) return null
  const fav = favorites.has(item.id) ? 1000 : 0
  const entry = history.find((h) => h.id === item.id)
  return base + frecencyBonus(entry, nowSec) + fav
}

export interface GroupedResults<T extends RankableItem> {
  sections: { title: string; items: T[] }[]
  flat: T[]
}

export function groupResults<T extends RankableItem>(
  sections: { title: string; candidates: T[] }[],
  q: string,
  history: HistoryEntry[],
  favorites: Set<string>,
  nowSec: number,
  perSection = 8
): GroupedResults<T> {
  const out: { title: string; items: T[] }[] = []
  const flat: T[] = []
  for (const { title, candidates } of sections) {
    const scored = candidates
      .map((item) => ({ item, score: rankItem(q, item, history, favorites, nowSec) }))
      .filter((x): x is { item: T; score: number } => x.score !== null)
      .sort((a, b) => b.score - a.score)
      .slice(0, perSection)
      .map((x) => x.item)
    if (scored.length) {
      out.push({ title, items: scored })
      flat.push(...scored)
    }
  }
  return { sections: out, flat }
}
