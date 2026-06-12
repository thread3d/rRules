import { useState, useEffect } from 'react'
import CodeMirror from '@uiw/react-codemirror'
import { javascript } from '@codemirror/lang-javascript'
import { oneDark } from '@codemirror/theme-one-dark'
import { EditorState } from '@codemirror/state'

export interface RuleEditorProps {
  value: string
  onChange: (value: string) => void
  height?: string
  readOnly?: boolean
}

export function RuleEditor({
  value,
  onChange,
  height = '400px',
  readOnly = false,
}: RuleEditorProps) {
  const [cmValue, setCmValue] = useState(value)

  useEffect(() => {
    if (value !== cmValue) {
      setCmValue(value)
    }
  }, [value])

  return (
    <div className="flex flex-col gap-2">
      <CodeMirror
        value={cmValue}
        height={height}
        theme={oneDark}
        extensions={[
          javascript({ typescript: true }),
          EditorState.readOnly.of(readOnly),
        ]}
        onChange={(val) => {
          setCmValue(val)
          onChange(val as string)
        }}
        basicSetup={{
          lineNumbers: true,
          foldGutter: true,
          highlightActiveLine: true,
          tabSize: 2,
        }}
      />
    </div>
  )
}

export default RuleEditor
