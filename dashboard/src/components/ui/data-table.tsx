import * as React from 'react'
import { ArrowUpDown, Search } from 'lucide-react'
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
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'

export type DataTableColumn<T> = {
  id: string
  header: React.ReactNode
  cell: (row: T) => React.ReactNode
  headerClassName?: string
  cellClassName?: string
}

export type DataTableSortOption = {
  value: string
  label: string
}

interface DataTableProps<T> {
  columns: Array<DataTableColumn<T>>
  rows: Array<T>
  getRowId: (row: T) => string
  loading?: boolean
  loadingLabel?: string
  emptyLabel?: string
  className?: string
  tableClassName?: string
  rowClassName?: string
  searchValue?: string
  onSearchValueChange?: (value: string) => void
  searchPlaceholder?: string
  sortValue?: string
  onSortValueChange?: (value: string) => void
  sortOptions?: Array<DataTableSortOption>
  controlsClassName?: string
}

export function DataTable<T>({
  columns,
  rows,
  getRowId,
  loading = false,
  loadingLabel = 'Loading…',
  emptyLabel = 'No rows to display.',
  className,
  tableClassName,
  rowClassName,
  searchValue,
  onSearchValueChange,
  searchPlaceholder = 'Search rows…',
  sortValue,
  onSortValueChange,
  sortOptions,
  controlsClassName,
}: DataTableProps<T>) {
  const showControls =
    onSearchValueChange !== undefined ||
    (onSortValueChange !== undefined && (sortOptions?.length ?? 0) > 0)

  return (
    <div className={cn('rounded-md border border-border/70', className)}>
      {showControls ? (
        <div
          className={cn(
            'flex flex-wrap items-center gap-3 border-b border-border/70 bg-background/40 px-4 py-3',
            controlsClassName,
          )}
        >
          {onSearchValueChange ? (
            <div className="relative min-w-[240px] flex-1 md:max-w-md">
              <Search className="pointer-events-none absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
              <Input
                value={searchValue ?? ''}
                onChange={(event) => {
                  onSearchValueChange(event.target.value)
                }}
                placeholder={searchPlaceholder}
                className="pl-9"
              />
            </div>
          ) : null}
          {onSortValueChange && sortOptions && sortOptions.length > 0 ? (
            <div className="flex items-center gap-2">
              <ArrowUpDown className="h-4 w-4 text-muted-foreground" />
              <Tabs value={sortValue} onValueChange={onSortValueChange}>
                <TabsList className="h-8">
                  {sortOptions.map((option) => (
                    <TabsTrigger
                      key={option.value}
                      value={option.value}
                      className="px-3"
                    >
                      {option.label}
                    </TabsTrigger>
                  ))}
                </TabsList>
              </Tabs>
            </div>
          ) : null}
        </div>
      ) : null}
      <Table className={tableClassName}>
        <TableHeader>
          <TableRow>
            {columns.map((column) => (
              <TableHead key={column.id} className={column.headerClassName}>
                {column.header}
              </TableHead>
            ))}
          </TableRow>
        </TableHeader>
        <TableBody>
          {loading ? (
            <TableRow>
              <TableCell
                colSpan={columns.length}
                className="text-muted-foreground"
              >
                {loadingLabel}
              </TableCell>
            </TableRow>
          ) : rows.length === 0 ? (
            <TableRow>
              <TableCell
                colSpan={columns.length}
                className="text-muted-foreground"
              >
                {emptyLabel}
              </TableCell>
            </TableRow>
          ) : (
            rows.map((row) => (
              <TableRow key={getRowId(row)} className={rowClassName}>
                {columns.map((column) => (
                  <TableCell
                    key={column.id}
                    className={column.cellClassName}
                  >
                    {column.cell(row)}
                  </TableCell>
                ))}
              </TableRow>
            ))
          )}
        </TableBody>
      </Table>
    </div>
  )
}
