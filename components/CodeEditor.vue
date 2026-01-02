<script setup lang="ts">
import { EditorState, Compartment } from '@codemirror/state'
import { EditorView, keymap, lineNumbers, highlightActiveLineGutter, highlightSpecialChars, drawSelection, dropCursor, rectangularSelection, crosshairCursor, highlightActiveLine } from '@codemirror/view'
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
import { linter, lintGutter, Diagnostic } from '@codemirror/lint'
import { autocompletion, completionKeymap, closeBrackets, closeBracketsKeymap, CompletionContext, type Completion } from '@codemirror/autocomplete'
import { tags } from '@lezer/highlight'

// Get variable store for autocomplete
const variableStore = useVariableStore()

// Template functions for autocomplete
const templateFunctions = [
  { name: '$hash.md5', snippet: '$hash.md5("value")', description: 'MD5 hash' },
  { name: '$hash.sha1', snippet: '$hash.sha1("value")', description: 'SHA1 hash' },
  { name: '$hash.sha256', snippet: '$hash.sha256("value")', description: 'SHA256 hash' },
  { name: '$hmac.sha256', snippet: '$hmac.sha256("value", "key")', description: 'HMAC-SHA256' },
  { name: '$base64.encode', snippet: '$base64.encode("value")', description: 'Base64 encode' },
  { name: '$base64.decode', snippet: '$base64.decode("encoded")', description: 'Base64 decode' },
  { name: '$url.encode', snippet: '$url.encode("value")', description: 'URL encode' },
  { name: '$url.decode', snippet: '$url.decode("encoded")', description: 'URL decode' },
  { name: '$encrypt', snippet: '$encrypt("key_name")', description: 'Get from keychain' },
  { name: '$uuid', snippet: '$uuid()', description: 'Generate UUID v4' },
  { name: '$timestamp', snippet: '$timestamp()', description: 'Unix timestamp' },
  { name: '$random.int', snippet: '$random.int(1, 100)', description: 'Random integer' },
  { name: '$random.string', snippet: '$random.string(16)', description: 'Random string' },
]

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
  language?: 'json' | 'xml' | 'html' | 'text' | 'javascript' | 'python' | 'rust' | 'java' | 'php' | 'shell' | 'go' | 'csharp' | 'ruby'
  readonly?: boolean
  placeholder?: string
  minHeight?: string
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: '',
  language: 'json',
  readonly: false,
  placeholder: '',
  minHeight: '200px',
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
    borderRadius: '6px',
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
    '& > ul > li[aria-selected]': {
      backgroundColor: 'hsl(240 3.7% 15.9%)',
      color: 'hsl(0 0% 98%)',
    },
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

const getLanguageExtension = (lang: string) => {
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
    autocompletion({
      override: [variableCompletions],
    }),
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
    languageCompartment.of(getLanguageExtension(props.language)),
    darkTheme,
    syntaxHighlighting(highlightStyle),
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
      effects: languageCompartment.reconfigure(getLanguageExtension(newLang)),
    })
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

defineExpose({
  formatJson,
  minifyJson,
})
</script>

<template>
  <div
    ref="editorContainer"
    class="code-editor h-full w-full"
    :style="{ minHeight }"
  />
</template>

<style>
.code-editor {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.code-editor .cm-editor {
  height: 100%;
  min-height: v-bind(minHeight);
  flex: 1;
  display: flex;
  flex-direction: column;
}

.code-editor .cm-scroller {
  overflow: auto !important;
  flex: 1;
}
</style>
