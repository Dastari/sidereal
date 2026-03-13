import { Check, MonitorCog, Moon, Sun } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { useTheme } from '@/hooks/use-theme'
import { gridIntensities, gridThemes } from '@/lib/grid-theme'

export function ThemeToggle() {
  const {
    theme,
    resolvedTheme,
    setTheme,
    gridTheme,
    setGridTheme,
    gridIntensity,
    setGridIntensity,
  } = useTheme()
  const activeGridTheme =
    gridThemes.find((entry) => entry.id === gridTheme) ?? gridThemes[0]

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          size="icon"
          className="relative overflow-hidden border-border/50 border-0! shadow-none! rounded-md"
        >
          <span
            aria-hidden="true"
            className="absolute inset-1 rounded-md opacity-80"
            style={{
              boxShadow: `0 0 16px ${activeGridTheme.accent}55`,
            }}
          />
          <span
            className="relative h-2.5 w-2.5 rounded-full border border-white/15"
            style={{
              backgroundColor: activeGridTheme.accent,
              boxShadow: `0 0 12px ${activeGridTheme.accent}`,
            }}
          />
          <span className="sr-only">Open theme settings</span>
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="grid-surface w-72">
        <DropdownMenuLabel className="flex items-center justify-between gap-3">
          <span>Display profile</span>
          <span className="text-[10px] uppercase tracking-[0.22em] text-muted-foreground">
            {activeGridTheme.label} / {gridIntensity}
          </span>
        </DropdownMenuLabel>
        <div className="px-2 pb-2 text-xs text-muted-foreground">
          Align the dashboard shell with GridCN theming while preserving the
          current browser color-scheme control.
        </div>
        <DropdownMenuSeparator />
        <DropdownMenuSub>
          <DropdownMenuSubTrigger>
            <MonitorCog className="mr-2 h-4 w-4" />
            Visual theme
          </DropdownMenuSubTrigger>
          <DropdownMenuSubContent className="grid-surface w-72">
            <DropdownMenuRadioGroup
              value={gridTheme}
              onValueChange={(value) => setGridTheme(value as typeof gridTheme)}
            >
              {gridThemes.map((entry) => (
                <DropdownMenuRadioItem key={entry.id} value={entry.id}>
                  <span
                    className="mr-2 h-2.5 w-2.5 rounded-full"
                    style={{
                      backgroundColor: entry.accent,
                      boxShadow: `0 0 10px ${entry.accent}`,
                    }}
                  />
                  <span className="flex flex-col gap-0.5">
                    <span>{entry.label}</span>
                    <span className="text-[11px] text-muted-foreground">
                      {entry.subtitle}
                    </span>
                  </span>
                </DropdownMenuRadioItem>
              ))}
            </DropdownMenuRadioGroup>
          </DropdownMenuSubContent>
        </DropdownMenuSub>
        <DropdownMenuSub>
          <DropdownMenuSubTrigger>
            <Check className="mr-2 h-4 w-4" />
            Intensity
          </DropdownMenuSubTrigger>
          <DropdownMenuSubContent className="grid-surface w-72">
            <DropdownMenuRadioGroup
              value={gridIntensity}
              onValueChange={(value) =>
                setGridIntensity(value as typeof gridIntensity)
              }
            >
              {gridIntensities.map((entry) => (
                <DropdownMenuRadioItem key={entry.id} value={entry.id}>
                  <span className="flex flex-col gap-0.5">
                    <span>{entry.label}</span>
                    <span className="text-[11px] text-muted-foreground">
                      {entry.description}
                    </span>
                  </span>
                </DropdownMenuRadioItem>
              ))}
            </DropdownMenuRadioGroup>
          </DropdownMenuSubContent>
        </DropdownMenuSub>
        <DropdownMenuSeparator />
        <DropdownMenuLabel>Browser color scheme</DropdownMenuLabel>
        <DropdownMenuRadioGroup
          value={theme}
          onValueChange={(value) => setTheme(value as typeof theme)}
        >
          <DropdownMenuRadioItem value="system">
            <MonitorCog className="mr-2 h-4 w-4" />
            System ({resolvedTheme})
          </DropdownMenuRadioItem>
          <DropdownMenuRadioItem value="dark">
            <Moon className="mr-2 h-4 w-4" />
            Dark
          </DropdownMenuRadioItem>
          <DropdownMenuRadioItem value="light">
            <Sun className="mr-2 h-4 w-4" />
            Light
          </DropdownMenuRadioItem>
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
