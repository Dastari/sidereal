import * as React from 'react'
import Prism from 'prismjs'
import { useEditable } from 'use-editable'
import { cn } from '@/lib/utils'

type ShaderCodeEditorProps = {
  value: string
  onChange: (value: string) => void
  className?: string
  placeholder?: string
  disabled?: boolean
}

const WGSL_KEYWORDS = [
  'alias',
  'bitcast',
  'break',
  'case',
  'const',
  'const_assert',
  'continue',
  'continuing',
  'default',
  'diagnostic',
  'discard',
  'else',
  'enable',
  'false',
  'fn',
  'for',
  'if',
  'let',
  'loop',
  'override',
  'requires',
  'return',
  'struct',
  'switch',
  'true',
  'var',
  'while',
]

const WGSL_BUILTINS = [
  'array',
  'atomic',
  'bool',
  'f16',
  'f32',
  'i32',
  'mat2x2',
  'mat2x3',
  'mat2x4',
  'mat3x2',
  'mat3x3',
  'mat3x4',
  'mat4x2',
  'mat4x3',
  'mat4x4',
  'ptr',
  'sampler',
  'sampler_comparison',
  'texture_1d',
  'texture_2d',
  'texture_2d_array',
  'texture_3d',
  'texture_cube',
  'texture_cube_array',
  'texture_depth_2d',
  'texture_depth_2d_array',
  'texture_depth_cube',
  'texture_depth_cube_array',
  'texture_depth_multisampled_2d',
  'texture_external',
  'texture_multisampled_2d',
  'texture_storage_1d',
  'texture_storage_2d',
  'texture_storage_2d_array',
  'texture_storage_3d',
  'u32',
  'vec2',
  'vec3',
  'vec4',
]

const WGSL_ATTRIBUTES = [
  'align',
  'binding',
  'builtin',
  'compute',
  'fragment',
  'group',
  'id',
  'interpolate',
  'invariant',
  'location',
  'must_use',
  'size',
  'vertex',
  'workgroup_size',
]

const wgslGrammar: Prism.Grammar = {
  comment: {
    pattern: /\/\/.*|\/\*[\s\S]*?\*\//,
    greedy: true,
  },
  attribute: {
    pattern: new RegExp(`@(?:${WGSL_ATTRIBUTES.join('|')})\\b`),
    alias: 'selector',
  },
  string: {
    pattern: /"(?:\\.|[^"\\])*"/,
    greedy: true,
  },
  number:
    /\b(?:0x[\da-fA-F]+(?:u|i)?|(?:\d+\.\d*|\d*\.\d+|\d+)(?:e[+-]?\d+)?(?:f|h|u|i)?)\b/,
  keyword: new RegExp(`\\b(?:${WGSL_KEYWORDS.join('|')})\\b`),
  builtin: new RegExp(`\\b(?:${WGSL_BUILTINS.join('|')})\\b`),
  function: /\b[a-zA-Z_]\w*(?=\s*\()/,
  operator: /[-+*/%]=?|[!=<>]=?|&&|\|\||->|[&|^~?:]/,
  punctuation: /[()[\]{};,.:]/,
}

Prism.languages.wgsl = wgslGrammar

type PrismNode = string | Prism.Token

function tokenClassName(type: string): string {
  switch (type) {
    case 'comment':
      return 'text-emerald-400/80'
    case 'keyword':
      return 'text-sky-300'
    case 'builtin':
      return 'text-violet-300'
    case 'function':
      return 'text-amber-200'
    case 'string':
      return 'text-orange-300'
    case 'number':
      return 'text-cyan-300'
    case 'attribute':
      return 'text-pink-300'
    case 'operator':
      return 'text-foreground/90'
    case 'punctuation':
      return 'text-foreground/70'
    default:
      return 'text-foreground'
  }
}

function renderToken(node: PrismNode, key: string): React.ReactNode {
  if (typeof node === 'string') {
    return <React.Fragment key={key}>{node}</React.Fragment>
  }

  const content = Array.isArray(node.content)
    ? node.content.map((child, index) => renderToken(child, `${key}-${index}`))
    : renderToken(node.content, `${key}-content`)

  return (
    <span key={key} className={tokenClassName(node.type)}>
      {content}
    </span>
  )
}

function highlightWgsl(source: string): React.ReactNode {
  const tokens = Prism.tokenize(source, wgslGrammar)
  return tokens.map((token, index) => renderToken(token, `token-${index}`))
}

export function ShaderCodeEditor({
  value,
  onChange,
  className,
  placeholder = '// Select a shader from the catalog or drop a .wgsl file here',
  disabled = false,
}: ShaderCodeEditorProps) {
  const editorRef = React.useRef<HTMLPreElement>(null)
  const gutterRef = React.useRef<HTMLDivElement>(null)
  const lineCount = Math.max(1, value.split('\n').length)
  const lineNumbers = React.useMemo(
    () => Array.from({ length: lineCount }, (_, index) => index + 1),
    [lineCount],
  )

  const handleEditorChange = React.useCallback(
    (nextValue: string) => {
      onChange(nextValue.replace(/\r\n/g, '\n'))
    },
    [onChange],
  )

  useEditable(editorRef, handleEditorChange, {
    disabled,
    indentation: 2,
  })

  const handleScroll = React.useCallback(() => {
    const editor = editorRef.current
    const gutter = gutterRef.current
    if (!editor || !gutter) return
    gutter.scrollTop = editor.scrollTop
  }, [])

  return (
    <div className={cn('flex min-h-0 flex-1 overflow-hidden', className)}>
      <div
        ref={gutterRef}
        className="select-none overflow-hidden border-r border-border-subtle bg-secondary/30 px-3 py-4 text-right font-mono text-xs leading-6 text-muted-foreground"
        aria-hidden="true"
      >
        {lineNumbers.map((lineNumber) => (
          <div key={lineNumber} className="h-6">
            {lineNumber}
          </div>
        ))}
      </div>
      <pre
        ref={editorRef}
        spellCheck={false}
        onScroll={handleScroll}
        className="shader-code-editor min-h-0 flex-1 overflow-auto bg-transparent px-5 py-4 font-mono text-sm leading-6 text-foreground outline-none"
        style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}
        data-placeholder={placeholder}
        suppressContentEditableWarning
      >
        {value.length > 0 ? highlightWgsl(value) : null}
      </pre>
    </div>
  )
}
