import { describe, expect, it } from 'vitest'
import { LOCAL_GROUP_KEY, groupApps, groupKeyOf } from '../src/lib/app-groups'
import type { AppView } from '../src/lib/types'

// Synthetic app entries only: no real software names or machine paths belong in
// this repository.
let nextId = 0
function app(name: string, path: string): AppView {
  nextId += 1
  return {
    id: `fixture-${nextId}`,
    name,
    path,
    args: [],
    workDir: '',
    autostart: false,
    autostartDelayMs: 0,
    skipIfRunning: true,
    hidden: false,
    tags: [],
    category: '',
    runAsAdmin: false,
    exists: true,
    icon: null
  }
}

describe('groupKeyOf', () => {
  it('treats relative paths as the local group with a blank label', () => {
    for (const path of ['Apps/Aurora/Aurora.exe', 'Apps\\Borealis\\Borealis.exe', 'tool.exe']) {
      expect(groupKeyOf(path)).toEqual({ key: LOCAL_GROUP_KEY, label: '', hint: '' })
    }
  })

  it('treats empty and whitespace-only paths as the local group', () => {
    expect(groupKeyOf('')).toEqual({ key: LOCAL_GROUP_KEY, label: '', hint: '' })
    expect(groupKeyOf('   ')).toEqual({ key: LOCAL_GROUP_KEY, label: '', hint: '' })
    expect(groupKeyOf(undefined as unknown as string)).toEqual({
      key: LOCAL_GROUP_KEY,
      label: '',
      hint: ''
    })
  })

  it('groups by drive plus the first two folders of the exe parent directory', () => {
    expect(groupKeyOf('E:\\Work\\Tools\\SomeApp\\SomeApp.exe')).toEqual({
      key: 'E:\\Work\\Tools',
      label: 'Work\\Tools',
      hint: 'E:\\Work\\Tools'
    })
  })

  it('keeps a single folder between the drive and the exe', () => {
    expect(groupKeyOf('D:\\Solo\\solo.exe')).toEqual({
      key: 'D:\\Solo',
      label: 'Solo',
      hint: 'D:\\Solo'
    })
  })

  it('normalizes separators and the drive letter', () => {
    expect(groupKeyOf('e:/work/tools/some-app/app.exe')).toEqual({
      key: 'E:\\work\\tools',
      label: 'work\\tools',
      hint: 'E:\\work\\tools'
    })
  })

  it('keeps the drive root as the group when the exe sits directly on it', () => {
    expect(groupKeyOf('E:\\portable.exe')).toEqual({
      key: 'E:\\',
      label: 'E:',
      hint: 'E:\\'
    })
  })

  it('keeps non-ascii folder names readable', () => {
    expect(groupKeyOf('E:\\示例盘\\工具集\\应用\\应用.exe')).toEqual({
      key: 'E:\\示例盘\\工具集',
      label: '示例盘\\工具集',
      hint: 'E:\\示例盘\\工具集'
    })
  })

  it('groups UNC paths by server and share', () => {
    expect(groupKeyOf('\\\\server\\share\\tools\\app.exe')).toEqual({
      key: '\\\\server\\share',
      label: 'server\\share',
      hint: '\\\\server\\share'
    })
  })
})

describe('groupApps', () => {
  it('returns an empty array for no apps', () => {
    expect(groupApps([])).toEqual([])
  })

  it('splits apps into source groups with the local group first', () => {
    const groups = groupApps([
      app('Cascade', 'D:\\Fixture\\Tools\\Cascade\\Cascade.exe'),
      app('Delta', 'E:\\Fixture\\Utilities\\Delta\\Delta.exe'),
      app('Aurora', 'D:\\Fixture\\Media\\Aurora\\Aurora.exe'),
      app('Local Tool', 'Apps/Local Tool/Local Tool.exe'),
      app('Borealis', 'D:\\Fixture\\Media\\Borealis\\Borealis.exe')
    ])

    expect(groups.map((group) => group.key)).toEqual([
      LOCAL_GROUP_KEY,
      'D:\\Fixture\\Media',
      'D:\\Fixture\\Tools',
      'E:\\Fixture\\Utilities'
    ])
    expect(groups.map((group) => group.label)).toEqual([
      '',
      'Fixture\\Media',
      'Fixture\\Tools',
      'Fixture\\Utilities'
    ])
    expect(groups.map((group) => group.hint)).toEqual([
      '',
      'D:\\Fixture\\Media',
      'D:\\Fixture\\Tools',
      'E:\\Fixture\\Utilities'
    ])
    expect(groups.map((group) => group.apps.length)).toEqual([1, 2, 1, 1])
    expect(groups.map((group) => group.apps.map((entry) => entry.name))).toEqual([
      ['Local Tool'],
      ['Aurora', 'Borealis'],
      ['Cascade'],
      ['Delta']
    ])
  })

  it('merges apps that share the same source folder', () => {
    const groups = groupApps([
      app('Aurora', 'D:\\Fixture\\Media\\Aurora\\Aurora.exe'),
      app('Borealis', 'D:\\Fixture\\Media\\Borealis\\Borealis.exe'),
      app('Cirrus', 'D:\\Fixture\\Media\\Cirrus\\Cirrus.exe')
    ])

    expect(groups).toHaveLength(1)
    expect(groups[0].key).toBe('D:\\Fixture\\Media')
    expect(groups[0].apps).toHaveLength(3)
  })

  it('merges subfolders under the same two-level source prefix', () => {
    const groups = groupApps([
      app('Alpha', 'E:\\Work\\Tools\\Alpha\\alpha.exe'),
      app('Beta', 'E:\\Work\\Tools\\Beta\\beta.exe'),
      app('Gamma', 'E:\\Work\\Elsewhere\\Gamma\\gamma.exe')
    ])

    expect(groups.map((group) => group.key)).toEqual([
      'E:\\Work\\Elsewhere',
      'E:\\Work\\Tools'
    ])
    expect(groups[1].apps.map((entry) => entry.name)).toEqual(['Alpha', 'Beta'])
  })

  it('collects relative paths, empty paths and bare names into one local group', () => {
    const groups = groupApps([
      app('Alpha', 'Apps/Alpha/Alpha.exe'),
      app('Beta', ''),
      app('Gamma', 'Apps\\Gamma\\Gamma.exe')
    ])

    expect(groups).toHaveLength(1)
    expect(groups[0].key).toBe(LOCAL_GROUP_KEY)
    expect(groups[0].label).toBe('')
    expect(groups[0].hint).toBe('')
    expect(groups[0].apps.map((entry) => entry.name)).toEqual(['Alpha', 'Beta', 'Gamma'])
  })

  it('sorts groups by label, case-insensitively, with local first', () => {
    const groups = groupApps([
      app('One', 'D:\\Zeta\\one.exe'),
      app('Two', 'D:\\alpha\\two.exe'),
      app('Three', 'D:\\Beta\\three.exe'),
      app('Four', 'Apps/Four/Four.exe')
    ])

    expect(groups.map((group) => group.label)).toEqual(['', 'alpha', 'Beta', 'Zeta'])
  })

  it('sorts the apps inside a group by name, case-insensitively', () => {
    const groups = groupApps([
      app('gamma', 'D:\\Fixture\\Media\\gamma\\gamma.exe'),
      app('Alpha', 'D:\\Fixture\\Media\\Alpha\\Alpha.exe'),
      app('beta', 'D:\\Fixture\\Media\\beta\\beta.exe')
    ])

    expect(groups[0].apps.map((entry) => entry.name)).toEqual(['Alpha', 'beta', 'gamma'])
  })

  it('merges the same folder written with different casing into one group', () => {
    const groups = groupApps([
      app('Alpha', 'E:\\Tools\\Aurora\\alpha.exe'),
      app('Beta', 'e:\\tools\\aurora\\beta.exe')
    ])

    expect(groups).toHaveLength(1)
    expect(groups[0].key).toBe('E:\\Tools\\Aurora')
    expect(groups[0].apps.map((entry) => entry.name)).toEqual(['Alpha', 'Beta'])
  })

  it('does not mutate the input array or its app order', () => {
    const input = [
      app('Cascade', 'D:\\Fixture\\Tools\\Cascade\\Cascade.exe'),
      app('Alpha', 'Apps/Alpha/Alpha.exe')
    ]
    const order = input.map((entry) => entry.name)

    groupApps(input)

    expect(input.map((entry) => entry.name)).toEqual(order)
  })
})
