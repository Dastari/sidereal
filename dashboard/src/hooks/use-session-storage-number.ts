import { useEffect, useState } from 'react'

export function useSessionStorageNumber(
  key: string,
  defaultValue: number,
): [number, (value: number) => void] {
  const [value, setValue] = useState(defaultValue)

  useEffect(() => {
    if (typeof window === 'undefined') {
      return
    }

    const storedValue = window.sessionStorage.getItem(key)
    if (storedValue == null) {
      return
    }

    const parsedValue = Number(storedValue)
    if (Number.isFinite(parsedValue)) {
      setValue(parsedValue)
    }
  }, [key])

  const setStoredValue = (nextValue: number) => {
    setValue(nextValue)
    if (typeof window === 'undefined') {
      return
    }
    window.sessionStorage.setItem(key, String(nextValue))
  }

  return [value, setStoredValue]
}
