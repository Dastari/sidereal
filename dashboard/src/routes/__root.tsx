import {
  ErrorComponent,
  HeadContent,
  Outlet,
  Scripts,
  createRootRoute,
} from '@tanstack/react-router'
import { NuqsAdapter } from 'nuqs/adapters/react'

import appCss from '../styles.css?url'
import { ThemeProvider } from '@/components/ThemeProvider'
import { TooltipProvider } from '@/components/ui/tooltip'

const THEME_STORAGE_KEY = 'sidereal-theme'
const THEME_INIT_SCRIPT = `(function(){try{var key='${THEME_STORAGE_KEY}';var stored=localStorage.getItem(key);var hasExplicit=stored==='light'||stored==='dark';var resolved=hasExplicit?stored:(window.matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light');var root=document.documentElement;root.classList.remove('light','dark');root.classList.add(resolved);root.style.colorScheme=resolved;root.dataset.theme=resolved;}catch(_){}})();`

export const Route = createRootRoute({
  head: () => ({
    meta: [
      { charSet: 'utf-8' },
      { name: 'viewport', content: 'width=device-width, initial-scale=1' },
      { title: 'Sidereal Explorer' },
      {
        name: 'description',
        content: 'Graph explorer for Sidereal game world',
      },
    ],
    links: [
      { rel: 'stylesheet', href: appCss },
      { rel: 'preconnect', href: 'https://fonts.googleapis.com' },
      {
        rel: 'preconnect',
        href: 'https://fonts.gstatic.com',
        crossOrigin: 'anonymous',
      },
      {
        rel: 'stylesheet',
        href: 'https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap',
      },
    ],
  }),
  notFoundComponent: () => (
    <div
      style={{
        minHeight: '100vh',
        display: 'grid',
        placeItems: 'center',
        background: '#0b0d12',
        color: '#d7e0ea',
        fontFamily: 'Inter, sans-serif',
      }}
    >
      <div style={{ textAlign: 'center' }}>
        <h1 style={{ marginBottom: 8 }}>Page Not Found</h1>
        <p style={{ opacity: 0.8 }}>
          The requested dashboard route does not exist.
        </p>
      </div>
    </div>
  ),
  errorComponent: ({ error }) => (
    <html lang="en" suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: THEME_INIT_SCRIPT }} />
        <HeadContent />
      </head>
      <body>
        <div
          style={{
            minHeight: '100vh',
            display: 'grid',
            placeItems: 'center',
            background: '#0b0d12',
            color: '#d7e0ea',
            fontFamily: 'Inter, sans-serif',
            padding: 24,
          }}
        >
          <div style={{ width: '100%', maxWidth: 720 }}>
            <h1 style={{ marginBottom: 12 }}>Dashboard Route Error</h1>
            <ErrorComponent error={error} />
          </div>
        </div>
        <Scripts />
      </body>
    </html>
  ),
  component: RootComponent,
})

function RootComponent() {
  return (
    <html lang="en" suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: THEME_INIT_SCRIPT }} />
        <HeadContent />
      </head>
      <body>
        <ThemeProvider defaultTheme="system">
          <TooltipProvider delayDuration={200}>
            <NuqsAdapter>
              <Outlet />
            </NuqsAdapter>
          </TooltipProvider>
        </ThemeProvider>
        <Scripts />
      </body>
    </html>
  )
}
