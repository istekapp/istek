import type { Config } from 'tailwindcss'

export default {
  darkMode: 'class',
  content: [],
  safelist: [
    // Protocol colors
    'text-protocol-http',
    'text-protocol-ws', 
    'text-protocol-sse',
    'text-protocol-graphql',
    'text-protocol-grpc',
    'text-protocol-mqtt',
    'text-protocol-unix',
    'text-protocol-mcp',
    'bg-protocol-http',
    'bg-protocol-ws',
    'bg-protocol-sse',
    'bg-protocol-graphql',
    'bg-protocol-grpc',
    'bg-protocol-mqtt',
    'bg-protocol-unix',
    'bg-protocol-mcp',
    // Method colors
    'text-method-get',
    'text-method-post',
    'text-method-put',
    'text-method-patch',
    'text-method-delete',
    'text-method-head',
    'text-method-options',
    'bg-method-get',
    'bg-method-post',
    'bg-method-put',
    'bg-method-patch',
    'bg-method-delete',
    'bg-method-head',
    'bg-method-options',
  ],
  theme: {
    extend: {
      fontFamily: {
        sans: ['Inter', 'system-ui', 'sans-serif'],
        mono: ['JetBrains Mono', 'monospace'],
      },
      fontSize: {
        'xs': ['0.8125rem', { lineHeight: '1.25rem' }],    // 13px
        'sm': ['0.875rem', { lineHeight: '1.375rem' }],    // 14px
        'base': ['0.9375rem', { lineHeight: '1.5rem' }],   // 15px
        'lg': ['1.0625rem', { lineHeight: '1.625rem' }],   // 17px
        'xl': ['1.1875rem', { lineHeight: '1.75rem' }],    // 19px
        '2xl': ['1.375rem', { lineHeight: '1.875rem' }],   // 22px
      },
      colors: {
        border: 'hsl(var(--border))',
        input: 'hsl(var(--input))',
        ring: 'hsl(var(--ring))',
        background: 'hsl(var(--background))',
        foreground: 'hsl(var(--foreground))',
        primary: {
          DEFAULT: 'hsl(var(--primary))',
          foreground: 'hsl(var(--primary-foreground))',
        },
        secondary: {
          DEFAULT: 'hsl(var(--secondary))',
          foreground: 'hsl(var(--secondary-foreground))',
        },
        destructive: {
          DEFAULT: 'hsl(var(--destructive))',
          foreground: 'hsl(var(--destructive-foreground))',
        },
        muted: {
          DEFAULT: 'hsl(var(--muted))',
          foreground: 'hsl(var(--muted-foreground))',
        },
        accent: {
          DEFAULT: 'hsl(var(--accent))',
          foreground: 'hsl(var(--accent-foreground))',
        },
        card: {
          DEFAULT: 'hsl(var(--card))',
          foreground: 'hsl(var(--card-foreground))',
        },
        popover: {
          DEFAULT: 'hsl(var(--popover))',
          foreground: 'hsl(var(--popover-foreground))',
        },
        // Pastel method colors
        method: {
          get: '#86efac',      // pastel green
          post: '#fde047',     // pastel yellow
          put: '#93c5fd',      // pastel blue
          patch: '#d8b4fe',    // pastel purple
          delete: '#fca5a5',   // pastel red
          head: '#a5f3fc',     // pastel cyan
          options: '#f9a8d4',  // pastel pink
        },
        // Protocol colors (pastel)
        protocol: {
          http: '#86efac',     // pastel green
          ws: '#d8b4fe',       // pastel purple
          sse: '#fb923c',      // pastel orange for SSE
          graphql: '#f9a8d4',  // pastel pink
          grpc: '#a5f3fc',     // pastel cyan
          mqtt: '#fdba74',     // pastel orange
          unix: '#a5b4fc',     // pastel indigo
          mcp: '#67e8f9',      // pastel cyan/teal for MCP
        },
      },
      borderRadius: {
        lg: 'var(--radius)',
        md: 'calc(var(--radius) - 2px)',
        sm: 'calc(var(--radius) - 4px)',
      },
    },
  },
  plugins: [],
} satisfies Config
