import { Moon, Sun } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { useTheme } from '@/hooks/use-theme'

export function ThemeToggle() {
  const { theme, resolvedTheme, setTheme } = useTheme()

  const cycleTheme = () => {
    const effectiveTheme = theme === 'system' ? resolvedTheme : theme
    setTheme(effectiveTheme === 'dark' ? 'light' : 'dark')
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button variant="ghost" size="icon" onClick={cycleTheme}>
          {resolvedTheme === 'light' ? (
            <Sun className="h-4 w-4" />
          ) : (
            <Moon className="h-4 w-4" />
          )}
          <span className="sr-only">Toggle theme</span>
        </Button>
      </TooltipTrigger>
      <TooltipContent>
        <p>Theme: {theme === 'system' ? `${resolvedTheme} (system)` : theme}</p>
      </TooltipContent>
    </Tooltip>
  )
}
