<script setup lang="ts">
import { EditorState, Compartment, RangeSetBuilder } from '@codemirror/state'
import { EditorView, keymap, lineNumbers, highlightActiveLineGutter, highlightSpecialChars, drawSelection, dropCursor, rectangularSelection, crosshairCursor, highlightActiveLine, Decoration, ViewPlugin, ViewUpdate, DecorationSet, WidgetType } from '@codemirror/view'
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands'
import { syntaxHighlighting, indentOnInput, bracketMatching, foldGutter, foldKeymap, HighlightStyle } from '@codemirror/language'
import { json, jsonParseLinter } from '@codemirror/lang-json'
import { xml } from '@codemirror/lang-xml'
import { html } from '@codemirror/lang-html'
import { javascript } from '@codemirror/lang-javascript'
import { python } from '@codemirror/lang-python'
import { rust } from '@codemirror/lang-rust'
import { java } from '@codemirror/lang-java'
import { php } from '@codemirror/lang-php'
import { go } from '@codemirror/lang-go'
import { StreamLanguage } from '@codemirror/language'
import { shell } from '@codemirror/legacy-modes/mode/shell'
import { ruby } from '@codemirror/legacy-modes/mode/ruby'
import { csharp } from '@codemirror/legacy-modes/mode/clike'
import { graphql as graphqlExtension, updateSchema } from 'cm6-graphql'
import { buildClientSchema, getIntrospectionQuery, type IntrospectionQuery } from 'graphql'
import { linter, lintGutter, Diagnostic } from '@codemirror/lint'
import { autocompletion, completionKeymap, closeBrackets, closeBracketsKeymap, CompletionContext, type Completion, type CompletionResult } from '@codemirror/autocomplete'
import { tags } from '@lezer/highlight'

// Get variable store for autocomplete
const variableStore = useVariableStore()

// Template functions for autocomplete
const templateFunctions = [
  { name: '$hash.md5', snippet: "$hash.md5('value')", description: 'MD5 hash' },
  { name: '$hash.sha1', snippet: "$hash.sha1('value')", description: 'SHA1 hash' },
  { name: '$hash.sha256', snippet: "$hash.sha256('value')", description: 'SHA256 hash' },
  { name: '$hmac.sha256', snippet: "$hmac.sha256('value', 'key')", description: 'HMAC-SHA256' },
  { name: '$base64.encode', snippet: "$base64.encode('value')", description: 'Base64 encode' },
  { name: '$base64.decode', snippet: "$base64.decode('encoded')", description: 'Base64 decode' },
  { name: '$url.encode', snippet: "$url.encode('value')", description: 'URL encode' },
  { name: '$url.decode', snippet: "$url.decode('encoded')", description: 'URL decode' },
  { name: '$uuid', snippet: '$uuid()', description: 'Generate UUID v4' },
  { name: '$timestamp', snippet: '$timestamp()', description: 'Unix timestamp' },
  { name: '$random.int', snippet: '$random.int(1, 100)', description: 'Random integer' },
  { name: '$random.string', snippet: '$random.string(16)', description: 'Random string' },
]

// istek API autocomplete for pre/post request scripts
const istekCompletions = (context: CompletionContext) => {
  const word = context.matchBefore(/[\w.]*/)
  if (!word || word.from === word.to) return null

  const text = word.text
  const options: Completion[] = []

  // Only provide completions if typing starts with 'istek' or 'i'
  if (text === 'i' || text === 'is' || text === 'ist' || text === 'iste' || text === 'istek') {
    options.push({
      label: 'istek',
      type: 'namespace',
      detail: 'Scripting API',
      boost: 10,
    })
  }
  
  // istek. completions
  if (text === 'istek.') {
    options.push(
      { label: 'istek.variables', type: 'property', detail: 'Variable operations', info: 'Get and set variables' },
      { label: 'istek.request', type: 'property', detail: 'Request data', info: 'Access request properties' },
      { label: 'istek.response', type: 'property', detail: 'Response data (post-request)', info: 'Access response data' },
      { label: 'istek.abort()', type: 'method', detail: 'Abort the request', info: 'Cancel the request execution' },
      { label: 'istek.environment', type: 'property', detail: 'Current environment name', info: 'string' },
    )
  }

  // istek.variables. completions
  if (text === 'istek.variables.') {
    options.push(
      { label: 'istek.variables.get', type: 'method', detail: '(name: string) => string | undefined', apply: 'istek.variables.get("")', info: 'Get a variable value by name' },
      { label: 'istek.variables.set', type: 'method', detail: '(name: string, value: string) => void', apply: 'istek.variables.set("", "")', info: 'Set a variable value' },
    )
  }

  // istek.request. completions
  if (text === 'istek.request.') {
    options.push(
      { label: 'istek.request.method', type: 'property', detail: 'string', info: 'HTTP method (GET, POST, etc.)' },
      { label: 'istek.request.url', type: 'property', detail: 'string', info: 'Request URL' },
      { label: 'istek.request.headers', type: 'property', detail: 'object', info: 'Request headers as key-value pairs' },
      { label: 'istek.request.params', type: 'property', detail: 'object', info: 'Query parameters as key-value pairs' },
      { label: 'istek.request.body', type: 'property', detail: 'string | undefined', info: 'Request body content' },
      { label: 'istek.request.setHeader', type: 'method', detail: '(name: string, value: string) => void', apply: 'istek.request.setHeader("", "")', info: 'Add or modify a request header' },
    )
  }

  // istek.response. completions
  if (text === 'istek.response.') {
    options.push(
      { label: 'istek.response.status', type: 'property', detail: 'number', info: 'HTTP status code (e.g., 200)' },
      { label: 'istek.response.statusText', type: 'property', detail: 'string', info: 'HTTP status text (e.g., "OK")' },
      { label: 'istek.response.body', type: 'property', detail: 'string', info: 'Response body as string' },
      { label: 'istek.response.time', type: 'property', detail: 'number', info: 'Response time in milliseconds' },
      { label: 'istek.response.headers', type: 'property', detail: 'object', info: 'Response headers as key-value pairs' },
      { label: 'istek.response.json', type: 'method', detail: '() => any', apply: 'istek.response.json()', info: 'Parse response body as JSON' },
    )
  }

  if (options.length === 0) return null

  return {
    from: word.from,
    options,
    validFor: /^[\w.]*$/,
  }
}

// Variable autocomplete source
const variableCompletions = (context: CompletionContext) => {
  // Check if we're inside {{ 
  const textBefore = context.state.sliceDoc(0, context.pos)
  const lastOpenBrace = textBefore.lastIndexOf('{{')
  const lastCloseBrace = textBefore.lastIndexOf('}}')
  
  if (lastOpenBrace === -1 || lastOpenBrace < lastCloseBrace) {
    return null
  }
  
  const searchTerm = textBefore.slice(lastOpenBrace + 2).toLowerCase()
  const from = lastOpenBrace
  
  const options: Completion[] = []
  
  // Add variables
  for (const [name, data] of variableStore.resolvedVariables.value) {
    if (name.toLowerCase().includes(searchTerm)) {
      options.push({
        label: `{{${name}}}`,
        type: 'variable',
        detail: data.isSecret ? '••••••' : String(data.value).slice(0, 30),
        apply: `{{${name}}}`,
        boost: 2,
      })
    }
  }
  
  // Add template functions
  for (const fn of templateFunctions) {
    if (fn.name.toLowerCase().includes(searchTerm) || fn.description.toLowerCase().includes(searchTerm)) {
      options.push({
        label: `{{${fn.name}}}`,
        type: 'function',
        detail: fn.description,
        apply: `{{${fn.snippet}}}`,
        boost: 1,
      })
    }
  }
  
  if (options.length === 0) return null
  
  return {
    from,
    options,
    validFor: /^(\{\{)?[\w.$]*$/,
  }
}

interface Props {
  modelValue: string
  language?: 'json' | 'xml' | 'html' | 'text' | 'javascript' | 'python' | 'rust' | 'java' | 'php' | 'shell' | 'go' | 'csharp' | 'ruby' | 'graphql'
  readonly?: boolean
  placeholder?: string
  minHeight?: string
  graphqlSchema?: any
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: '',
  language: 'json',
  readonly: false,
  placeholder: '',
  minHeight: '200px',
  graphqlSchema: undefined,
})

const emit = defineEmits<{
  'update:modelValue': [value: string]
  'validation': [valid: boolean, errors: string[]]
}>()

const editorContainer = ref<HTMLElement>()
const editorView = ref<EditorView>()
const languageCompartment = new Compartment()

// Custom dark theme
const darkTheme = EditorView.theme({
  '&': {
    backgroundColor: 'hsl(240 10% 3.9%)',
    color: 'hsl(0 0% 98%)',
    fontSize: '14px',
    fontFamily: "'JetBrains Mono', monospace",
    height: '100%',
  },
  '.cm-content': {
    caretColor: 'hsl(0 0% 98%)',
    padding: '12px 0',
  },
  '.cm-scroller': {
    overflow: 'auto',
    fontFamily: "'JetBrains Mono', monospace",
  },
  '.cm-cursor, .cm-dropCursor': {
    borderLeftColor: 'hsl(0 0% 98%)',
  },
  '&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection': {
    backgroundColor: 'hsl(240 3.7% 25%)',
  },
  '.cm-panels': {
    backgroundColor: 'hsl(240 10% 3.9%)',
    color: 'hsl(0 0% 98%)',
  },
  '.cm-panels.cm-panels-top': {
    borderBottom: '1px solid hsl(240 3.7% 15.9%)',
  },
  '.cm-panels.cm-panels-bottom': {
    borderTop: '1px solid hsl(240 3.7% 15.9%)',
  },
  '.cm-searchMatch': {
    backgroundColor: 'hsl(45 100% 50% / 0.3)',
    outline: '1px solid hsl(45 100% 50% / 0.5)',
  },
  '.cm-searchMatch.cm-searchMatch-selected': {
    backgroundColor: 'hsl(45 100% 50% / 0.5)',
  },
  '.cm-activeLine': {
    backgroundColor: 'hsl(240 3.7% 10%)',
  },
  '.cm-selectionMatch': {
    backgroundColor: 'hsl(240 3.7% 20%)',
  },
  '&.cm-focused .cm-matchingBracket, &.cm-focused .cm-nonmatchingBracket': {
    backgroundColor: 'hsl(240 3.7% 25%)',
    outline: '1px solid hsl(240 3.7% 35%)',
  },
  '.cm-gutters': {
    backgroundColor: 'hsl(240 10% 3.9%)',
    color: 'hsl(240 5% 45%)',
    border: 'none',
    borderRight: '1px solid hsl(240 3.7% 15.9%)',
  },
  '.cm-activeLineGutter': {
    backgroundColor: 'hsl(240 3.7% 10%)',
    color: 'hsl(0 0% 70%)',
  },
  '.cm-foldPlaceholder': {
    backgroundColor: 'hsl(240 3.7% 15.9%)',
    color: 'hsl(240 5% 64.9%)',
    border: 'none',
  },
  '.cm-tooltip': {
    backgroundColor: 'hsl(240 10% 8%)',
    border: '1px solid hsl(240 3.7% 15.9%)',
    borderRadius: '8px',
    boxShadow: '0 10px 15px -3px rgba(0, 0, 0, 0.4), 0 4px 6px -4px rgba(0, 0, 0, 0.3)',
    overflow: 'hidden',
  },
  '.cm-tooltip .cm-tooltip-arrow:before': {
    borderTopColor: 'hsl(240 3.7% 15.9%)',
    borderBottomColor: 'hsl(240 3.7% 15.9%)',
  },
  '.cm-tooltip .cm-tooltip-arrow:after': {
    borderTopColor: 'hsl(240 10% 8%)',
    borderBottomColor: 'hsl(240 10% 8%)',
  },
  '.cm-tooltip-autocomplete': {
    minWidth: '320px',
    '& > ul': {
      fontFamily: "'JetBrains Mono', monospace",
      fontSize: '13px',
      padding: '4px',
      maxHeight: '280px',
    },
    '& > ul > li': {
      padding: '8px 12px',
      borderRadius: '6px',
      margin: '2px 0',
      display: 'flex',
      alignItems: 'center',
      gap: '10px',
      lineHeight: '1.4',
    },
    '& > ul > li[aria-selected]': {
      backgroundColor: 'hsl(240 3.7% 15.9%)',
      color: 'hsl(0 0% 98%)',
    },
    '& > ul > li:hover:not([aria-selected])': {
      backgroundColor: 'hsl(240 3.7% 12%)',
    },
  },
  '.cm-completionIcon': {
    fontSize: '14px',
    opacity: 1,
    width: '20px',
    textAlign: 'center',
    padding: '0',
    marginRight: '4px',
  },
  '.cm-completionIcon-variable': {
    '&::after': {
      content: '"x"',
      color: 'hsl(262 83% 58%)',
      fontWeight: 'bold',
    },
  },
  '.cm-completionIcon-function': {
    '&::after': {
      content: '"f"',
      color: 'hsl(142 71% 45%)',
      fontWeight: 'bold',
    },
  },
  '.cm-completionIcon-method': {
    '&::after': {
      content: '"m"',
      color: 'hsl(199 89% 48%)',
      fontWeight: 'bold',
    },
  },
  '.cm-completionLabel': {
    flex: 1,
    fontWeight: 500,
  },
  '.cm-completionDetail': {
    color: 'hsl(240 5% 50%)',
    fontSize: '12px',
    marginLeft: 'auto',
    fontStyle: 'normal',
    maxWidth: '150px',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    whiteSpace: 'nowrap',
  },
  // Custom completion item styling
  '.cm-completion-item-custom': {
    display: 'flex',
    alignItems: 'center',
    gap: '10px',
    width: '100%',
  },
  '.cm-completion-icon-custom': {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    width: '20px',
    height: '20px',
    flexShrink: 0,
  },
  '.cm-completion-label-custom': {
    flex: 1,
    fontFamily: "'JetBrains Mono', monospace",
    fontSize: '13px',
    fontWeight: 500,
  },
  '.cm-completion-detail-custom': {
    color: 'hsl(240 5% 50%)',
    fontSize: '12px',
    marginLeft: 'auto',
    maxWidth: '120px',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    whiteSpace: 'nowrap',
    flexShrink: 0,
  },
  '.cm-lintRange-error': {
    backgroundImage: 'none',
    borderBottom: '2px wavy hsl(0 84.2% 60.2%)',
  },
  '.cm-lintRange-warning': {
    backgroundImage: 'none',
    borderBottom: '2px wavy hsl(45 93% 47%)',
  },
  '.cm-diagnostic': {
    padding: '8px 12px',
    borderRadius: '4px',
  },
  '.cm-diagnostic-error': {
    backgroundColor: 'hsl(0 62.8% 20%)',
    borderLeft: '4px solid hsl(0 84.2% 60.2%)',
  },
  '.cm-diagnostic-warning': {
    backgroundColor: 'hsl(45 62.8% 20%)',
    borderLeft: '4px solid hsl(45 93% 47%)',
  },
}, { dark: true })

// Syntax highlighting colors
const highlightStyle = HighlightStyle.define([
  { tag: tags.keyword, color: '#c678dd' },
  { tag: tags.operator, color: '#56b6c2' },
  { tag: tags.special(tags.variableName), color: '#e06c75' },
  { tag: tags.typeName, color: '#e5c07b' },
  { tag: tags.atom, color: '#d19a66' },
  { tag: tags.number, color: '#d19a66' },
  { tag: tags.definition(tags.variableName), color: '#61afef' },
  { tag: tags.string, color: '#98c379' },
  { tag: tags.special(tags.string), color: '#98c379' },
  { tag: tags.comment, color: '#5c6370', fontStyle: 'italic' },
  { tag: tags.variableName, color: '#e06c75' },
  { tag: tags.tagName, color: '#e06c75' },
  { tag: tags.bracket, color: '#abb2bf' },
  { tag: tags.meta, color: '#abb2bf' },
  { tag: tags.attributeName, color: '#d19a66' },
  { tag: tags.attributeValue, color: '#98c379' },
  { tag: tags.propertyName, color: '#e06c75' },
  { tag: tags.bool, color: '#d19a66' },
  { tag: tags.null, color: '#d19a66' },
])

// Known template function names
const knownFunctionNames = [
  'sensitive', 'hash.md5', 'hash.sha1', 'hash.sha256', 'hash.sha512',
  'hmac.sha256', 'hmac.sha512', 'base64.encode', 'base64.decode',
  'url.encode', 'url.decode', 'uuid', 'timestamp', 'timestamp.ms',
  'random.int', 'random.float', 'random.string', 'random.hex',
]

// Check if a function name is known
const isKnownFunction = (name: string): boolean => {
  const cleanName = name.startsWith('$') ? name.slice(1) : name
  const fnMatch = cleanName.match(/^([\w.]+)\s*\(/)
  if (fnMatch) {
    return knownFunctionNames.includes(fnMatch[1])
  }
  return false
}

// State for sensitive value popover
const sensitivePopover = ref<{
  show: boolean
  x: number
  y: number
  value: string
  revealed: boolean
}>({
  show: false,
  x: 0,
  y: 0,
  value: '',
  revealed: false,
})

const showSensitivePopover = (event: MouseEvent, value: string) => {
  const rect = (event.target as HTMLElement).getBoundingClientRect()
  sensitivePopover.value = {
    show: true,
    x: rect.left,
    y: rect.bottom + 4,
    value,
    revealed: false,
  }
}

const hideSensitivePopover = () => {
  sensitivePopover.value.show = false
  sensitivePopover.value.revealed = false
}

const toggleReveal = () => {
  sensitivePopover.value.revealed = !sensitivePopover.value.revealed
}

// Widget to replace function content with condensed version
class FunctionWidget extends WidgetType {
  constructor(
    readonly functionName: string, 
    readonly isSensitive: boolean,
    readonly originalValue: string
  ) {
    super()
  }
  
  toDOM() {
    const span = document.createElement('span')
    span.className = this.isSensitive 
      ? 'cm-sensitive-function' 
      : 'cm-template-function'
    span.textContent = `{{$${this.functionName}(...)}}`
    
    if (this.isSensitive) {
      span.style.cursor = 'pointer'
      span.addEventListener('mouseenter', (e) => {
        showSensitivePopover(e as MouseEvent, this.originalValue)
      })
      span.addEventListener('mouseleave', () => {
        // Delay hiding to allow moving to popover
        setTimeout(() => {
          const popoverEl = document.querySelector('.sensitive-popover')
          if (popoverEl && !popoverEl.matches(':hover')) {
            hideSensitivePopover()
          }
        }, 100)
      })
    } else {
      span.title = `Function: ${this.functionName}`
    }
    
    return span
  }
  
  ignoreEvent() { return false }
}

// Decoration marks for variables and functions
const variableValidMark = Decoration.mark({ class: 'cm-variable-valid' })
const variableInvalidMark = Decoration.mark({ class: 'cm-variable-invalid' })
const functionMark = Decoration.mark({ class: 'cm-template-function' })

// Create decorations for variables and template functions
const createVariableDecorations = (view: EditorView): DecorationSet => {
  const builder = new RangeSetBuilder<Decoration>()
  const doc = view.state.doc.toString()
  
  // Match {{...}} patterns
  const regex = /\{\{([^}]*)\}\}/g
  let match
  
  while ((match = regex.exec(doc)) !== null) {
    const varName = match[1].trim()
    const from = match.index
    const to = match.index + match[0].length
    
    // Check if it's a function
    if (isKnownFunction(varName)) {
      // Extract function name and args
      const cleanName = varName.startsWith('$') ? varName.slice(1) : varName
      const fnMatch = cleanName.match(/^([\w.]+)\s*\((.*)\)$/)
      const functionName = fnMatch ? fnMatch[1] : cleanName
      const functionArgs = fnMatch ? fnMatch[2] : ''
      const isSensitive = functionName === 'sensitive'
      
      // Extract the actual value from args (remove quotes)
      let originalValue = functionArgs
      const valueMatch = functionArgs.match(/^['"](.*)['"]$/)
      if (valueMatch) {
        originalValue = valueMatch[1]
      }
      
      // Replace with widget for condensed display
      builder.add(from, to, Decoration.replace({
        widget: new FunctionWidget(functionName, isSensitive, originalValue),
      }))
    } else {
      // It's a variable - check if resolved
      const isValid = variableStore.resolvedVariables.value.has(varName)
      builder.add(from, to, isValid ? variableValidMark : variableInvalidMark)
    }
  }
  
  return builder.finish()
}

// ViewPlugin to apply decorations
const variableHighlighter = ViewPlugin.fromClass(class {
  decorations: DecorationSet
  
  constructor(view: EditorView) {
    this.decorations = createVariableDecorations(view)
  }
  
  update(update: ViewUpdate) {
    if (update.docChanged || update.viewportChanged) {
      this.decorations = createVariableDecorations(update.view)
    }
  }
}, {
  decorations: v => v.decorations,
})

// Theme for variable/function highlighting
const variableHighlightTheme = EditorView.baseTheme({
  '.cm-variable-valid': {
    color: '#60a5fa',
    backgroundColor: 'rgba(59, 130, 246, 0.2)',
    borderRadius: '2px',
    padding: '0 2px',
  },
  '.cm-variable-invalid': {
    color: '#f87171',
    backgroundColor: 'rgba(239, 68, 68, 0.2)',
    borderRadius: '2px',
    padding: '0 2px',
  },
  '.cm-template-function': {
    color: '#c084fc',
    backgroundColor: 'rgba(168, 85, 247, 0.2)',
    borderRadius: '2px',
    padding: '0 2px',
    cursor: 'help',
  },
  '.cm-sensitive-function': {
    color: '#c084fc',
    backgroundColor: 'rgba(168, 85, 247, 0.2)',
    borderRadius: '2px',
    padding: '0 2px',
    cursor: 'help',
  },
})

const getLanguageExtension = (lang: string, schema?: any) => {
  switch (lang) {
    case 'json':
      return json()
    case 'xml':
      return xml()
    case 'html':
      return html()
    case 'javascript':
      return javascript()
    case 'python':
      return python()
    case 'rust':
      return rust()
    case 'java':
      return java()
    case 'php':
      return php()
    case 'go':
      return go()
    case 'shell':
      return StreamLanguage.define(shell)
    case 'csharp':
      return StreamLanguage.define(csharp)
    case 'ruby':
      return StreamLanguage.define(ruby)
    case 'graphql':
      return graphqlExtension(schema)
    default:
      return []
  }
}

// Custom JSON linter with better error messages
const customJsonLinter = linter((view) => {
  const diagnostics: Diagnostic[] = []
  const content = view.state.doc.toString()
  
  if (!content.trim()) {
    emit('validation', true, [])
    return diagnostics
  }
  
  try {
    JSON.parse(content)
    emit('validation', true, [])
  } catch (e: any) {
    const match = e.message.match(/position (\d+)/)
    let pos = match ? parseInt(match[1]) : 0
    
    // Find line for error position
    let line = view.state.doc.lineAt(Math.min(pos, view.state.doc.length))
    
    diagnostics.push({
      from: line.from,
      to: line.to,
      severity: 'error',
      message: e.message.replace(/^JSON\.parse: /, ''),
    })
    
    emit('validation', false, [e.message])
  }
  
  return diagnostics
})

const getLinter = (lang: string) => {
  if (lang === 'json') {
    return customJsonLinter
  }
  return []
}

const createEditorState = (content: string) => {
  // Custom autocomplete options for better styling
  const autocompleteOptions = {
    activateOnTyping: true,
    icons: true,
    addToOptions: [
      {
        render: (completion: Completion) => {
          const wrapper = document.createElement('div')
          wrapper.className = 'cm-completion-item-custom'
          
          // Icon based on type
          const icon = document.createElement('span')
          icon.className = 'cm-completion-icon-custom'
          if (completion.type === 'variable') {
            icon.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="color: hsl(262 83% 58%);"><path d="M8 21s-4-3-4-9 4-9 4-9"/><path d="M16 3s4 3 4 9-4 9-4 9"/><line x1="15" x2="9" y1="9" y2="15"/><line x1="9" x2="15" y1="9" y2="15"/></svg>'
          } else if (completion.type === 'function') {
            icon.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="color: hsl(142 71% 45%);"><rect width="18" height="18" x="3" y="3" rx="2" ry="2"/><path d="M9 17c2 0 2.8-1 2.8-2.8V10c0-2 1-3.3 3.2-3"/><path d="M9 11.2h5.7"/></svg>'
          } else {
            icon.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="color: hsl(199 89% 48%);"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/></svg>'
          }
          wrapper.appendChild(icon)
          
          // Label
          const label = document.createElement('span')
          label.className = 'cm-completion-label-custom'
          label.textContent = completion.label
          wrapper.appendChild(label)
          
          // Detail
          if (completion.detail) {
            const detail = document.createElement('span')
            detail.className = 'cm-completion-detail-custom'
            detail.textContent = completion.detail
            wrapper.appendChild(detail)
          }
          
          return wrapper
        },
        position: 20
      }
    ]
  }

  // Configure autocomplete based on language
  let autocompleteConfig
  if (props.language === 'graphql') {
    // Let cm6-graphql handle completions
    autocompleteConfig = autocompletion({
      ...autocompleteOptions,
      override: [],
    })
  } else if (props.language === 'javascript') {
    // JavaScript with istek API completions for pre/post request scripts
    autocompleteConfig = autocompletion({
      ...autocompleteOptions,
      override: [istekCompletions, variableCompletions],
    })
  } else {
    // Default: variable completions for template syntax
    autocompleteConfig = autocompletion({
      ...autocompleteOptions,
      override: [variableCompletions],
    })
  }

  const extensions = [
    lineNumbers(),
    highlightActiveLineGutter(),
    highlightSpecialChars(),
    history(),
    foldGutter(),
    drawSelection(),
    dropCursor(),
    EditorState.allowMultipleSelections.of(true),
    indentOnInput(),
    bracketMatching(),
    closeBrackets(),
    autocompleteConfig,
    rectangularSelection(),
    crosshairCursor(),
    highlightActiveLine(),
    keymap.of([
      ...closeBracketsKeymap,
      ...defaultKeymap,
      ...historyKeymap,
      ...foldKeymap,
      ...completionKeymap,
      indentWithTab,
    ]),
    languageCompartment.of(getLanguageExtension(props.language, props.graphqlSchema)),
    darkTheme,
    syntaxHighlighting(highlightStyle),
    variableHighlighter,
    variableHighlightTheme,
    lintGutter(),
    getLinter(props.language),
    EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        emit('update:modelValue', update.state.doc.toString())
      }
    }),
    EditorView.lineWrapping,
    EditorState.readOnly.of(props.readonly),
  ]

  return EditorState.create({
    doc: content,
    extensions,
  })
}

onMounted(() => {
  if (editorContainer.value) {
    editorView.value = new EditorView({
      state: createEditorState(props.modelValue),
      parent: editorContainer.value,
    })
  }
})

onUnmounted(() => {
  editorView.value?.destroy()
})

// Watch for external value changes
watch(() => props.modelValue, (newValue) => {
  if (editorView.value && newValue !== editorView.value.state.doc.toString()) {
    editorView.value.dispatch({
      changes: {
        from: 0,
        to: editorView.value.state.doc.length,
        insert: newValue,
      },
    })
  }
})

// Watch for language changes
watch(() => props.language, (newLang) => {
  if (editorView.value) {
    editorView.value.dispatch({
      effects: languageCompartment.reconfigure(getLanguageExtension(newLang, props.graphqlSchema)),
    })
  }
})

// Watch for GraphQL schema changes
watch(() => props.graphqlSchema, (newSchema) => {
  if (editorView.value && props.language === 'graphql' && newSchema) {
    updateSchema(editorView.value, newSchema)
  }
})

// Format JSON
const formatJson = () => {
  if (props.language !== 'json' || !editorView.value) return
  
  try {
    const content = editorView.value.state.doc.toString()
    const formatted = JSON.stringify(JSON.parse(content), null, 2)
    editorView.value.dispatch({
      changes: {
        from: 0,
        to: editorView.value.state.doc.length,
        insert: formatted,
      },
    })
  } catch {
    // Invalid JSON, can't format
  }
}

// Minify JSON
const minifyJson = () => {
  if (props.language !== 'json' || !editorView.value) return
  
  try {
    const content = editorView.value.state.doc.toString()
    const minified = JSON.stringify(JSON.parse(content))
    editorView.value.dispatch({
      changes: {
        from: 0,
        to: editorView.value.state.doc.length,
        insert: minified,
      },
    })
  } catch {
    // Invalid JSON, can't minify
  }
}

// Update GraphQL schema externally
const updateGraphQLSchema = (schema: any) => {
  if (editorView.value && props.language === 'graphql') {
    updateSchema(editorView.value, schema)
  }
}

defineExpose({
  formatJson,
  minifyJson,
  updateGraphQLSchema,
  getEditorView: () => editorView.value,
})
</script>

<template>
  <div class="relative w-full h-full">
    <div
      ref="editorContainer"
      class="code-editor h-full w-full"
      :style="{ minHeight }"
    />
    
    <!-- Sensitive Value Hover Popover -->
    <Teleport to="body">
      <div
        v-if="sensitivePopover.show"
        class="sensitive-popover fixed z-50 rounded-lg border border-border bg-popover shadow-lg p-3 min-w-[200px]"
        :style="{ left: sensitivePopover.x + 'px', top: sensitivePopover.y + 'px' }"
        @mouseenter="sensitivePopover.show = true"
        @mouseleave="hideSensitivePopover"
      >
        <div class="flex items-center gap-2 text-xs text-muted-foreground mb-2">
          <Icon name="lucide:lock" class="h-3 w-3 text-purple-500" />
          <span>Sensitive Value</span>
        </div>
        <div class="flex items-center gap-2">
          <code class="flex-1 px-2 py-1 rounded bg-muted font-mono text-sm">
            {{ sensitivePopover.revealed ? sensitivePopover.value : '••••••••' }}
          </code>
          <button
            class="p-1.5 rounded hover:bg-accent transition-colors"
            :title="sensitivePopover.revealed ? 'Hide value' : 'Reveal value'"
            @click="toggleReveal"
          >
            <Icon 
              :name="sensitivePopover.revealed ? 'lucide:eye-off' : 'lucide:eye'" 
              class="h-4 w-4 text-muted-foreground" 
            />
          </button>
        </div>
      </div>
    </Teleport>
    
  </div>
</template>

<style>
.code-editor {
  display: flex;
  flex-direction: column;
  overflow: hidden;
  width: 100%;
}

.code-editor .cm-editor {
  height: 100%;
  width: 100%;
  min-height: v-bind(minHeight);
  flex: 1;
  display: flex;
  flex-direction: column;
}

.code-editor .cm-scroller {
  overflow: auto !important;
  flex: 1;
}

.code-editor .cm-content {
  width: 100%;
}
</style>
