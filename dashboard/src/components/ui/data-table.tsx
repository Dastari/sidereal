import * as React from 'react'
import { ArrowDown, ArrowUp, ArrowUpDown, Columns3, Search } from 'lucide-react'
import { cn } from '@/lib/utils'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { HUDFrame } from '@/components/ui/hud-frame'

export type DataTableSortDirection = 'asc' | 'desc'

export type DataTableSortState = {
  columnId: string
  direction: DataTableSortDirection
}

export type DataTableColumn<T> = {
  id: string
  header: React.ReactNode
  cell: (row: T) => React.ReactNode
  sortAccessor?: (row: T) => string | number | boolean | null | undefined
  sortable?: boolean
  enableHiding?: boolean
  defaultVisible?: boolean
  width?: number
  minWidth?: number
  maxWidth?: number
  headerClassName?: string
  cellClassName?: string
}

export type DataTableSelectionMode = 'none' | 'single' | 'multiple'
export type DataTableRowClassName<T> =
  | string
  | ((row: T, context: { selected: boolean; selectable: boolean }) => string)

export type DataTableActionBarContext<T> = {
  selectedRows: Array<T>
  selectedRowIds: Set<string>
  clearSelection: () => void
  visibleColumnIds: Set<string>
}

interface DataTableProps<T> {
  columns: Array<DataTableColumn<T>>
  rows: Array<T>
  getRowId: (row: T) => string
  loading?: boolean
  loadingLabel?: string
  emptyLabel?: string
  getSearchText?: (row: T, visibleColumns: Array<DataTableColumn<T>>) => string
  className?: string
  tableClassName?: string
  rowClassName?: DataTableRowClassName<T>
  searchValue?: string
  onSearchValueChange?: (value: string) => void
  searchPlaceholder?: string
  sortState?: DataTableSortState | null
  onSortStateChange?: (value: DataTableSortState | null) => void
  defaultSortState?: DataTableSortState | null
  controlsClassName?: string
  paginationMode?: 'none' | 'pagination' | 'infinite'
  defaultPageSize?: number
  pageSizeOptions?: Array<number>
  page?: number
  onPageChange?: (page: number) => void
  pageSize?: number
  onPageSizeChange?: (pageSize: number) => void
  hasMore?: boolean
  isLoadingMore?: boolean
  onLoadMore?: () => void | Promise<void>
  selectionMode?: DataTableSelectionMode
  selectedRowIds?: Set<string> | Array<string>
  onSelectedRowIdsChange?: (selected: Set<string>) => void
  isRowSelectable?: (row: T) => boolean
  actionBar?: (context: DataTableActionBarContext<T>) => React.ReactNode
  showColumnVisibilityToggle?: boolean
}

function toComparable(value: unknown): string | number {
  if (typeof value === 'number') return value
  if (typeof value === 'boolean') return value ? 1 : 0
  if (value == null) return ''
  return String(value).toLowerCase()
}

export function DataTable<T>({
  columns,
  rows,
  getRowId,
  loading = false,
  loadingLabel = 'Loading...',
  emptyLabel = 'No rows to display.',
  getSearchText,
  className,
  tableClassName,
  rowClassName,
  searchValue,
  onSearchValueChange,
  searchPlaceholder = 'Search rows...',
  sortState,
  onSortStateChange,
  defaultSortState = null,
  controlsClassName,
  paginationMode = 'none',
  defaultPageSize = 20,
  pageSizeOptions = [10, 20, 50, 100],
  page,
  onPageChange,
  pageSize,
  onPageSizeChange,
  hasMore = false,
  isLoadingMore = false,
  onLoadMore,
  selectionMode = 'none',
  selectedRowIds,
  onSelectedRowIdsChange,
  isRowSelectable,
  actionBar,
  showColumnVisibilityToggle = true,
}: DataTableProps<T>) {
  const [internalSearch, setInternalSearch] = React.useState('')
  const effectiveSearch = searchValue ?? internalSearch
  const setSearch = onSearchValueChange ?? setInternalSearch

  const [internalSortState, setInternalSortState] =
    React.useState<DataTableSortState | null>(defaultSortState)
  const effectiveSortState = sortState ?? internalSortState
  const setSortState = onSortStateChange ?? setInternalSortState

  const [internalVisibleColumnIds, setInternalVisibleColumnIds] =
    React.useState<Set<string>>(
      () =>
        new Set(
          columns
            .filter((column) => column.defaultVisible !== false)
            .map((column) => column.id),
        ),
    )
  React.useEffect(() => {
    setInternalVisibleColumnIds((current) => {
      const next = new Set<string>()
      for (const column of columns) {
        if (current.has(column.id)) next.add(column.id)
        else if (column.defaultVisible !== false && !current.has(column.id)) {
          next.add(column.id)
        }
      }
      return next
    })
  }, [columns])
  const visibleColumnIds = internalVisibleColumnIds

  const [internalColumnWidths, setInternalColumnWidths] = React.useState<
    Record<string, number>
  >(() => {
    const initial: Record<string, number> = {}
    for (const column of columns) {
      if (typeof column.width === 'number') {
        initial[column.id] = column.width
      }
    }
    return initial
  })

  const [internalSelectedRowIds, setInternalSelectedRowIds] = React.useState<
    Set<string>
  >(new Set())
  const effectiveSelectedRowIds = React.useMemo(() => {
    if (selectedRowIds instanceof Set) return new Set(selectedRowIds)
    if (Array.isArray(selectedRowIds)) return new Set(selectedRowIds)
    return internalSelectedRowIds
  }, [internalSelectedRowIds, selectedRowIds])

  const setSelectedRowIds = React.useCallback(
    (next: Set<string>) => {
      if (onSelectedRowIdsChange) {
        onSelectedRowIdsChange(new Set(next))
      } else {
        setInternalSelectedRowIds(new Set(next))
      }
    },
    [onSelectedRowIdsChange],
  )

  const visibleColumns = React.useMemo(
    () => columns.filter((column) => visibleColumnIds.has(column.id)),
    [columns, visibleColumnIds],
  )

  const filteredRows = React.useMemo(() => {
    const needle = effectiveSearch.trim().toLowerCase()
    if (!needle) return rows
    return rows.filter((row) => {
      const haystack = (
        getSearchText
          ? getSearchText(row, visibleColumns)
          : visibleColumns
              .map((column) => {
                if (column.sortAccessor) return column.sortAccessor(row)
                const rowAsRecord = row as Record<string, unknown>
                return rowAsRecord[column.id]
              })
              .map((value) => String(value ?? '').toLowerCase())
              .join(' ')
      ).toLowerCase()
      return haystack.includes(needle)
    })
  }, [effectiveSearch, getSearchText, rows, visibleColumns])

  const sortedRows = React.useMemo(() => {
    if (!effectiveSortState) return filteredRows
    const sortColumn = columns.find(
      (column) => column.id === effectiveSortState.columnId,
    )
    if (!sortColumn) return filteredRows
    const directionMultiplier = effectiveSortState.direction === 'asc' ? 1 : -1
    return [...filteredRows].sort((left, right) => {
      const leftValue = sortColumn.sortAccessor
        ? sortColumn.sortAccessor(left)
        : (left as Record<string, unknown>)[sortColumn.id]
      const rightValue = sortColumn.sortAccessor
        ? sortColumn.sortAccessor(right)
        : (right as Record<string, unknown>)[sortColumn.id]
      const leftComparable = toComparable(leftValue)
      const rightComparable = toComparable(rightValue)
      if (
        typeof leftComparable === 'number' &&
        typeof rightComparable === 'number'
      ) {
        return (leftComparable - rightComparable) * directionMultiplier
      }
      return (
        String(leftComparable).localeCompare(String(rightComparable)) *
        directionMultiplier
      )
    })
  }, [columns, effectiveSortState, filteredRows])

  const [internalPage, setInternalPage] = React.useState(1)
  const [internalPageSize, setInternalPageSize] =
    React.useState(defaultPageSize)
  const effectivePage = page ?? internalPage
  const effectivePageSize = pageSize ?? internalPageSize

  React.useEffect(() => {
    if (paginationMode !== 'pagination') return
    const maxPage = Math.max(
      1,
      Math.ceil(sortedRows.length / Math.max(effectivePageSize, 1)),
    )
    if (effectivePage > maxPage) {
      if (onPageChange) onPageChange(maxPage)
      else setInternalPage(maxPage)
    }
  }, [
    effectivePage,
    effectivePageSize,
    onPageChange,
    paginationMode,
    sortedRows.length,
  ])

  const paginatedRows = React.useMemo(() => {
    if (paginationMode !== 'pagination') return sortedRows
    const start = (effectivePage - 1) * effectivePageSize
    return sortedRows.slice(start, start + effectivePageSize)
  }, [effectivePage, effectivePageSize, paginationMode, sortedRows])

  const renderedRows = paginatedRows

  const selectedRows = React.useMemo(
    () => rows.filter((row) => effectiveSelectedRowIds.has(getRowId(row))),
    [effectiveSelectedRowIds, getRowId, rows],
  )

  const actionBarContext = React.useMemo<DataTableActionBarContext<T>>(
    () => ({
      selectedRows,
      selectedRowIds: effectiveSelectedRowIds,
      clearSelection: () => setSelectedRowIds(new Set()),
      visibleColumnIds,
    }),
    [
      effectiveSelectedRowIds,
      selectedRows,
      setSelectedRowIds,
      visibleColumnIds,
    ],
  )

  const infiniteContainerRef = React.useRef<HTMLDivElement | null>(null)
  const infiniteSentinelRef = React.useRef<HTMLDivElement | null>(null)
  React.useEffect(() => {
    if (paginationMode !== 'infinite' || !hasMore || !onLoadMore) return
    const root = infiniteContainerRef.current
    const sentinel = infiniteSentinelRef.current
    if (!root || !sentinel) return
    const observer = new IntersectionObserver(
      (entries) => {
        const [entry] = entries
        if (!entry.isIntersecting) return
        if (isLoadingMore) return
        void onLoadMore()
      },
      { root, rootMargin: '160px 0px', threshold: 0.01 },
    )
    observer.observe(sentinel)
    return () => observer.disconnect()
  }, [hasMore, isLoadingMore, onLoadMore, paginationMode])

  const canSelectRow = React.useCallback(
    (row: T) => (isRowSelectable ? isRowSelectable(row) : true),
    [isRowSelectable],
  )

  const toggleRowSelection = React.useCallback(
    (row: T) => {
      if (selectionMode === 'none') return
      const rowId = getRowId(row)
      if (!canSelectRow(row)) return
      if (selectionMode === 'single') {
        const next = new Set<string>()
        if (!effectiveSelectedRowIds.has(rowId)) next.add(rowId)
        setSelectedRowIds(next)
        return
      }
      const next = new Set(effectiveSelectedRowIds)
      if (next.has(rowId)) next.delete(rowId)
      else next.add(rowId)
      setSelectedRowIds(next)
    },
    [
      canSelectRow,
      effectiveSelectedRowIds,
      getRowId,
      selectionMode,
      setSelectedRowIds,
    ],
  )

  const selectableRenderedRowIds = React.useMemo(
    () =>
      renderedRows
        .filter((row) => canSelectRow(row))
        .map((row) => getRowId(row)),
    [canSelectRow, getRowId, renderedRows],
  )

  const allRenderedSelected =
    selectableRenderedRowIds.length > 0 &&
    selectableRenderedRowIds.every((rowId) =>
      effectiveSelectedRowIds.has(rowId),
    )

  const canToggleAll = selectionMode === 'multiple'

  const toggleAllRenderedRows = React.useCallback(() => {
    if (!canToggleAll) return
    const next = new Set(effectiveSelectedRowIds)
    if (allRenderedSelected) {
      for (const rowId of selectableRenderedRowIds) next.delete(rowId)
    } else {
      for (const rowId of selectableRenderedRowIds) next.add(rowId)
    }
    setSelectedRowIds(next)
  }, [
    allRenderedSelected,
    canToggleAll,
    effectiveSelectedRowIds,
    selectableRenderedRowIds,
    setSelectedRowIds,
  ])

  const hideableColumns = React.useMemo(
    () => columns.filter((column) => column.enableHiding !== false),
    [columns],
  )

  const onColumnResizeStart = React.useCallback(
    (event: React.MouseEvent, column: DataTableColumn<T>) => {
      event.preventDefault()
      const startX = event.clientX
      const startingWidth = Object.hasOwn(internalColumnWidths, column.id)
        ? internalColumnWidths[column.id]
        : (column.width ?? Math.max(column.minWidth ?? 120, 120))
      const minWidth = column.minWidth ?? 80
      const maxWidth = column.maxWidth ?? 960

      const onMouseMove = (moveEvent: MouseEvent) => {
        const delta = moveEvent.clientX - startX
        const nextWidth = Math.min(
          maxWidth,
          Math.max(minWidth, Math.round(startingWidth + delta)),
        )
        setInternalColumnWidths((current) => ({
          ...current,
          [column.id]: nextWidth,
        }))
      }

      const onMouseUp = () => {
        window.removeEventListener('mousemove', onMouseMove)
        window.removeEventListener('mouseup', onMouseUp)
      }

      window.addEventListener('mousemove', onMouseMove)
      window.addEventListener('mouseup', onMouseUp)
    },
    [internalColumnWidths],
  )

  const updateSortForColumn = React.useCallback(
    (column: DataTableColumn<T>) => {
      if (!column.sortable && !column.sortAccessor) return
      if (effectiveSortState?.columnId !== column.id) {
        setSortState({ columnId: column.id, direction: 'asc' })
        return
      }
      if (effectiveSortState.direction === 'asc') {
        setSortState({ columnId: column.id, direction: 'desc' })
        return
      }
      setSortState(null)
    },
    [effectiveSortState, setSortState],
  )

  const tableBody = (
    <Table className={cn('table-fixed', tableClassName)}>
      <TableHeader>
        <TableRow>
          {selectionMode !== 'none' ? (
            <TableHead className="w-[42px] px-2">
              {selectionMode === 'multiple' ? (
                <input
                  type="checkbox"
                  checked={allRenderedSelected}
                  onChange={() => toggleAllRenderedRows()}
                  aria-label="Select all rows"
                />
              ) : null}
            </TableHead>
          ) : null}
          {visibleColumns.map((column) => {
            const isSortable = column.sortable === true || !!column.sortAccessor
            const activeSort = effectiveSortState?.columnId === column.id
            const width = internalColumnWidths[column.id] ?? column.width
            return (
              <TableHead
                key={column.id}
                className={cn('relative', column.headerClassName)}
                style={
                  width
                    ? {
                        width,
                        minWidth: width,
                        maxWidth: width,
                      }
                    : {
                        minWidth: column.minWidth ?? 120,
                      }
                }
              >
                <div className="flex items-center gap-1">
                  {isSortable ? (
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      className="h-auto items-center gap-1 px-0 py-0 text-left hover:text-foreground"
                      onClick={() => updateSortForColumn(column)}
                    >
                      <span>{column.header}</span>
                      {activeSort ? (
                        effectiveSortState.direction === 'asc' ? (
                          <ArrowUp className="h-3.5 w-3.5" />
                        ) : (
                          <ArrowDown className="h-3.5 w-3.5" />
                        )
                      ) : (
                        <ArrowUpDown className="h-3.5 w-3.5 opacity-60" />
                      )}
                    </Button>
                  ) : (
                    <span>{column.header}</span>
                  )}
                </div>
                <Button
                  type="button"
                  variant="ghost"
                  className="absolute right-0 top-0 h-full w-1.5 min-w-0 cursor-col-resize rounded-none border-0 px-0 py-0 opacity-0 transition-opacity hover:opacity-100"
                  onMouseDown={(event) => onColumnResizeStart(event, column)}
                  aria-label={`Resize ${String(column.header)} column`}
                />
              </TableHead>
            )
          })}
        </TableRow>
      </TableHeader>
      <TableBody>
        {loading ? (
          <TableRow>
            <TableCell
              colSpan={
                visibleColumns.length + (selectionMode !== 'none' ? 1 : 0)
              }
              className="text-muted-foreground"
            >
              {loadingLabel}
            </TableCell>
          </TableRow>
        ) : renderedRows.length === 0 ? (
          <TableRow>
            <TableCell
              colSpan={
                visibleColumns.length + (selectionMode !== 'none' ? 1 : 0)
              }
              className="text-muted-foreground"
            >
              {emptyLabel}
            </TableCell>
          </TableRow>
        ) : (
          renderedRows.map((row) => {
            const rowId = getRowId(row)
            const selected = effectiveSelectedRowIds.has(rowId)
            const selectable = canSelectRow(row)
            const resolvedRowClassName =
              typeof rowClassName === 'function'
                ? rowClassName(row, { selected, selectable })
                : rowClassName
            return (
              <TableRow
                key={rowId}
                className={resolvedRowClassName}
                data-state={selected ? 'selected' : undefined}
                onClick={(event) => {
                  const target = event.target as HTMLElement | null
                  if (
                    target?.closest(
                      'button,input,a,textarea,select,label,[role="button"]',
                    )
                  ) {
                    return
                  }
                  if (selectionMode !== 'none' && selectable) {
                    toggleRowSelection(row)
                  }
                }}
              >
                {selectionMode !== 'none' ? (
                  <TableCell className="w-[42px] px-2">
                    <input
                      type="checkbox"
                      checked={selected}
                      disabled={!selectable}
                      onChange={() => toggleRowSelection(row)}
                      onClick={(event) => event.stopPropagation()}
                      aria-label={`Select row ${rowId}`}
                    />
                  </TableCell>
                ) : null}
                {visibleColumns.map((column) => {
                  const width = internalColumnWidths[column.id] ?? column.width
                  return (
                    <TableCell
                      key={column.id}
                      className={column.cellClassName}
                      style={
                        width
                          ? {
                              width,
                              minWidth: width,
                              maxWidth: width,
                            }
                          : undefined
                      }
                    >
                      {column.cell(row)}
                    </TableCell>
                  )
                })}
              </TableRow>
            )
          })
        )}
      </TableBody>
    </Table>
  )

  const totalPages = Math.max(
    1,
    Math.ceil(sortedRows.length / Math.max(effectivePageSize, 1)),
  )

  return (
    <HUDFrame className={className}>
      <div
        className={cn(
          'flex flex-wrap items-center gap-3 border-b border-border/70 bg-background/40 px-4 py-3',
          controlsClassName,
        )}
      >
        <div className="relative min-w-[240px] flex-1 md:max-w-md">
          <Search className="pointer-events-none absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={effectiveSearch}
            onChange={(event) => {
              setSearch(event.target.value)
            }}
            placeholder={searchPlaceholder}
            className="pl-9"
          />
        </div>

        {showColumnVisibilityToggle && hideableColumns.length > 0 ? (
          <details className="relative">
            <summary className="list-none">
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="gap-2"
              >
                <Columns3 className="h-4 w-4" />
                Columns
              </Button>
            </summary>
            <HUDFrame className="absolute right-0 z-20 mt-2 min-w-[220px] p-2 shadow-lg">
              {hideableColumns.map((column) => {
                const visible = visibleColumnIds.has(column.id)
                const canHide = column.enableHiding !== false
                return (
                  <label
                    key={column.id}
                    className="flex cursor-pointer items-center gap-2 rounded px-2 py-1.5 text-sm hover:bg-muted/40"
                  >
                    <input
                      type="checkbox"
                      checked={visible}
                      disabled={!canHide}
                      onChange={(event) => {
                        setInternalVisibleColumnIds((current) => {
                          const next = new Set(current)
                          if (event.target.checked) next.add(column.id)
                          else next.delete(column.id)
                          return next.size > 0 ? next : current
                        })
                      }}
                    />
                    <span>{column.header}</span>
                  </label>
                )
              })}
            </HUDFrame>
          </details>
        ) : null}

        {selectionMode !== 'none' ? (
          <Badge variant="secondary" className="tabular-nums">
            {effectiveSelectedRowIds.size} selected
          </Badge>
        ) : null}

        {actionBar ? (
          <div className="ml-auto">{actionBar(actionBarContext)}</div>
        ) : null}
      </div>

      {paginationMode === 'infinite' ? (
        <div ref={infiniteContainerRef} className="max-h-[36rem] overflow-auto">
          {tableBody}
          <div ref={infiniteSentinelRef} className="h-1 w-full" />
          {hasMore ? (
            <div className="flex items-center justify-center p-3 text-xs text-muted-foreground">
              {isLoadingMore ? 'Loading more...' : 'Scroll to load more'}
            </div>
          ) : null}
        </div>
      ) : (
        tableBody
      )}

      {paginationMode === 'pagination' ? (
        <div className="flex flex-wrap items-center gap-2 border-t border-border/70 bg-background/40 px-4 py-3">
          <div className="text-xs text-muted-foreground">
            Page {effectivePage} of {totalPages} ({sortedRows.length} rows)
          </div>
          <div className="ml-auto flex items-center gap-2">
            <select
              className="h-8 rounded-md border border-input bg-background px-2 text-xs"
              value={effectivePageSize}
              onChange={(event) => {
                const next = Number(event.target.value)
                if (onPageSizeChange) onPageSizeChange(next)
                else setInternalPageSize(next)
                if (onPageChange) onPageChange(1)
                else setInternalPage(1)
              }}
            >
              {pageSizeOptions.map((option) => (
                <option key={option} value={option}>
                  {option}/page
                </option>
              ))}
            </select>
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={effectivePage <= 1}
              onClick={() => {
                const next = Math.max(1, effectivePage - 1)
                if (onPageChange) onPageChange(next)
                else setInternalPage(next)
              }}
            >
              Previous
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={effectivePage >= totalPages}
              onClick={() => {
                const next = Math.min(totalPages, effectivePage + 1)
                if (onPageChange) onPageChange(next)
                else setInternalPage(next)
              }}
            >
              Next
            </Button>
          </div>
        </div>
      ) : null}
    </HUDFrame>
  )
}
