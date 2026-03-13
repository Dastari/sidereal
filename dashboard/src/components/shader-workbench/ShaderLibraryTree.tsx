import * as React from 'react'
import { ChevronDown, ChevronRight, FileCode2, Sparkles } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import { cn } from '@/lib/utils'

export type ShaderCatalogEntry = {
  shaderId: string
  filename: string
  shaderClass: 'fullscreen' | 'sprite' | 'effect' | 'unknown'
  assetId: string | null
  shaderRole: string | null
  bootstrapRequired: boolean | null
  dependencies: Array<string>
  sourcePath: string
  cachePath: string | null
  sourceExists: boolean
  cacheExists: boolean
  byteLength: number
  updatedAt: string
}

interface ShaderLibraryTreeProps {
  shaders: Array<ShaderCatalogEntry>
  selectedShaderId: string | null
  onSelect: (shaderId: string) => void
  search: string
}

export function ShaderLibraryTree({
  shaders,
  selectedShaderId,
  onSelect,
  search,
}: ShaderLibraryTreeProps) {
  const [openGroups, setOpenGroups] = React.useState<Record<string, boolean>>({})

  const grouped = React.useMemo(() => {
    const groups = new Map<string, Array<ShaderCatalogEntry>>()
    const needle = search.trim().toLowerCase()
    const filtered = !needle
      ? shaders
      : shaders.filter((entry) =>
          `${entry.filename} ${entry.sourcePath} ${entry.shaderClass} ${entry.assetId ?? ''} ${entry.shaderRole ?? ''} ${entry.dependencies.join(' ')}`
            .toLowerCase()
            .includes(needle),
        )

    for (const shader of filtered) {
      const key = shader.shaderClass
      const existing = groups.get(key)
      if (existing) {
        existing.push(shader)
      } else {
        groups.set(key, [shader])
      }
    }

    for (const items of groups.values()) {
      items.sort((left, right) => left.filename.localeCompare(right.filename))
    }

    return Array.from(groups.entries()).sort(([left], [right]) =>
      left.localeCompare(right),
    )
  }, [search, shaders])

  const isGroupOpen = React.useCallback(
    (key: string) => openGroups[key] ?? true,
    [openGroups],
  )

  const toggleGroup = React.useCallback((key: string) => {
    setOpenGroups((prev) => ({ ...prev, [key]: !(prev[key] ?? true) }))
  }, [])

  return (
    <ScrollArea className="h-full">
      <div className="space-y-2 p-4">
        <div className="flex items-center gap-2 rounded-md border border-border-subtle bg-secondary/20 px-3 py-2 text-sm text-muted-foreground">
          <Sparkles className="h-4 w-4 text-primary" />
          <span>Shader Assets</span>
          <span className="ml-auto text-xs">{shaders.length}</span>
        </div>
        {grouped.map(([groupKey, items]) => {
          const open = isGroupOpen(groupKey)
          return (
            <Collapsible
              key={groupKey}
              open={open}
              onOpenChange={() => toggleGroup(groupKey)}
            >
              <CollapsibleTrigger className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm font-medium text-foreground/90 transition-colors hover:bg-secondary/50">
                {open ? (
                  <ChevronDown className="h-4 w-4 text-muted-foreground" />
                ) : (
                  <ChevronRight className="h-4 w-4 text-muted-foreground" />
                )}
                <FileCode2 className="h-4 w-4 text-primary" />
                <span className="capitalize">{groupKey}</span>
                <span className="ml-auto text-xs text-muted-foreground">
                  {items.length}
                </span>
              </CollapsibleTrigger>
              <CollapsibleContent>
                <div className="ml-4 space-y-1 border-l border-border-subtle pl-3">
                  {items.map((entry) => (
                    <Button
                      key={entry.shaderId}
                      type="button"
                      variant="ghost"
                      className={cn(
                        'h-auto w-full justify-start rounded-md px-3 py-2 text-left transition-colors',
                        entry.shaderId === selectedShaderId
                          ? 'bg-primary/10 text-primary'
                          : 'hover:bg-secondary/50 text-foreground/85',
                      )}
                      onClick={() => onSelect(entry.shaderId)}
                    >
                      <div className="flex items-start gap-2">
                        <FileCode2 className="mt-0.5 h-4 w-4 shrink-0 text-primary/80" />
                        <div className="min-w-0 flex-1">
                          <div className="truncate text-sm font-medium">
                            {entry.filename}
                          </div>
                          <div className="truncate text-xs text-muted-foreground">
                            {entry.sourcePath}
                          </div>
                        </div>
                      </div>
                    </Button>
                  ))}
                </div>
              </CollapsibleContent>
            </Collapsible>
          )
        })}
        {grouped.length === 0 ? (
          <div className="px-2 py-6 text-center text-sm text-muted-foreground">
            No shaders matched the current search.
          </div>
        ) : null}
      </div>
    </ScrollArea>
  )
}
