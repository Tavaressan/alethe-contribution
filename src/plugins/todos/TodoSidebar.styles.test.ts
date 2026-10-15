import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { describe, expect, it } from 'vitest'

const css = readFileSync(resolve('src/plugins/todos/TodoSidebar.module.css'), 'utf8')

function rule(selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  return css.match(new RegExp(`${escaped} \\{([^}]*)\\}`))?.[1] ?? ''
}

describe('TodoSidebar styles', () => {
  it('wraps task titles and aligns row controls with the first line', () => {
    const title = rule('.todoTitleText')

    expect(title).toContain('overflow-wrap: anywhere')
    expect(title).not.toMatch(/white-space:\s*nowrap/)
    expect(title).not.toMatch(/text-overflow:\s*ellipsis/)
    expect(rule('.todoRow')).toMatch(/align-items:\s*start/)
    expect(rule('.todoTitle')).toContain('margin-top: 4px')
    expect(css).toMatch(/\.editInput\s*\{[^}]*height:\s*26px;[^}]*margin-top:\s*3px;/)
    expect(rule('.dragHandle')).toContain('margin-top: 4px')
    expect(rule('.checkButton')).toContain('margin-top: 7.5px')
    expect(rule('.rowActions')).toContain('margin-top: 6px')
  })

  it('preserves centered single-line rows in clean visual style', () => {
    expect(css).toMatch(
      /:global\(\[data-visual-style='clean'\]\) \.dragHandle,[\s\S]*?:global\(\[data-visual-style='clean'\]\) \.todoTitle\s*\{\s*margin-top: 1px;/,
    )
    expect(css).toMatch(
      /:global\(\[data-visual-style='clean'\]\) \.checkButton\s*\{\s*margin-top: 4\.5px;/,
    )
    expect(css).toMatch(
      /:global\(\[data-visual-style='clean'\]\) \.rowActions\s*\{\s*margin-top: 3px;/,
    )
    expect(css).toMatch(
      /:global\(\[data-visual-style='clean'\]\) \.editInput\s*\{\s*margin-top: 0;/,
    )
  })
})
