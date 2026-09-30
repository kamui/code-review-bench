import { expect, test } from 'bun:test'
import { skillReleaseLabel } from './data'

test('skill metadata omits unknown versions and distinguishes release dates from commit dates', () => {
  const skillReleases = [{ version: null, date: '2026-09-07', dateSource: 'commit', provenanceUrl: '/provenance' }] satisfies Parameters<typeof skillReleaseLabel>[0][number]['skillReleases']
  expect(skillReleaseLabel([{ skillReleases }])).toBe('Updated 2026-09-07')
  expect(skillReleaseLabel([{ skillReleases: [{ version: '1.2.3', date: '2026-09-28', dateSource: 'release', provenanceUrl: '/release' }] }])).toBe('1.2.3 · Released 2026-09-28')
  expect(skillReleaseLabel([{ skillReleases }, { skillReleases }])).toBe('Updated 2026-09-07')
})
